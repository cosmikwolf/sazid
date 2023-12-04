use tokio_postgres::NoTls;

use self::{
  openai_embeddings::{create_embedding, EmbeddingModel},
  types::EmbeddingVector,
  vector_db::{VectorDB, VectorDBConfig},
};

use super::errors::SazidError;

pub mod openai_embeddings;
pub mod types;
pub mod vector_db;

pub struct EmbeddingsManager {
  db: VectorDB,
  model: EmbeddingModel,
}

impl EmbeddingsManager {
  async fn init(db_config: &str, model: EmbeddingModel) -> Result<Self, SazidError> {
    let (client, connection) = tokio_postgres::connect(db_config, NoTls).await?;

    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("Connection error: {}", e);
      }
    });

    let db = VectorDB { client, config: VectorDBConfig { optimize_threads: 4 } };

    db.enable_extension().await?;
    Ok(EmbeddingsManager { db, model })
  }

  pub async fn drop_all_embeddings_tables(&self) -> Result<(), SazidError> {
    // a method that will drop all tables that have a suffix of _embeddings
    let query = "SELECT table_name FROM information_schema.tables WHERE table_name LIKE '%_embeddings';";
    let rows = self.db.client.query(query, &[]).await?;
    println!("rows: {:?}", rows);
    for row in rows {
      let category: String = row.get(0);
      self.db.client.batch_execute(format!("DROP TABLE IF EXISTS {}_embeddings CASCADE;", category).as_str()).await?;
    }

    Ok(())
  }

  pub async fn list_embeddings_categories(&self) -> Result<Vec<String>, SazidError> {
    Ok(vec![])
  }

  // Function to insert text and generate its embedding into the correct category table
  pub async fn add_text_embedding(&self, category_name: &str, text: &str) -> Result<(), SazidError> {
    let embedding_response = create_embedding(text.to_string(), &self.model.clone()).await.map_err(SazidError::from)?;
    let embeddings: Vec<Vec<f64>> = embedding_response
      .data
      .into_iter()
      .map(|d| d.embedding.iter().map(|e| *e as f64).collect::<Vec<f64>>())
      .collect();

    if let Some(embedding) = embeddings.first() {
      match self
        .db
        .insert_text_embedding(
          category_name,
          EmbeddingVector::new(embedding.to_vec(), text.to_string(), category_name.into()),
        )
        .await
      {
        Ok(_) => Ok(()),
        Err(e) => Err(SazidError::from(e)),
      }
    } else {
      Err(SazidError::Other("Failed to generate embedding".to_string()))
    }
  }
}
