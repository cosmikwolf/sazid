use tokio_postgres::NoTls;

use crate::{cli::Cli, config::Config};

use self::{
  embeddings_models::EmbeddingModel,
  types::{Embedding, EmbeddingData, PlainTextEmbedding, TextFileEmbeddingData},
  vector_db::{VectorDB, VectorDBConfig},
};
use super::errors::SazidError;
use dialoguer;

pub mod embeddings_models;
pub mod types;
pub mod vector_db;

pub struct EmbeddingsManager {
  db: VectorDB,
  model: EmbeddingModel,
}

impl EmbeddingsManager {
  pub async fn run(db_config: &str, args: Cli, config: Config) -> Result<Option<String>, SazidError> {
    let model = EmbeddingModel::Ada002(config.session_config.openai_config);
    let embeddings_manager = Self::init(db_config, model).await?;
    println!("args: {:#?}", args);
    Ok(match args {
      Cli { list_embeddings: true, .. } => {
        // let categories = embeddings_manager.list_embeddings_categories().await?;
        let embeddings = embeddings_manager.list_embeddings().await?;
        Some(format!("{:?}", embeddings))
      },
      Cli { delete_all_embeddings: true, .. } => {
        // ask the user to type 'yes' before proceeding
        // import dialoguer
        let confirm = dialoguer::Confirm::new()
          .with_prompt("Are you sure you want to delete all embeddings?")
          .interact()
          .map_err(SazidError::from)?;
        match confirm {
          true => {
            embeddings_manager.drop_all_embeddings_tables().await?;
            Some("deleting all embeddings tables".to_string())
          },
          false => Some("cancelled".to_string()),
        }
      },
      Cli { parse_source_embeddings: Some(_), .. } => {
        embeddings_manager.drop_all_embeddings_tables().await?;
        // embeddings_manager.parse_source_file_embeddings().await?;
        Some("parse_source_embeddings".to_string())
      },
      Cli { load_text_file_embeddings: Some(filepath), .. } => {
        // read the file at filepath
        let text = std::fs::read_to_string(filepath)?;
        embeddings_manager.add_text_embedding("text", &text).await?;
        Some("load_text_file_embeddings".to_string())
      },
      Cli { load_text_embeddings: Some(text), .. } => {
        embeddings_manager.add_text_embedding("text", &text).await?;
        Some("load_text_embeddings".to_string())
      },
      _ => None,
    })
  }

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

  pub async fn list_embeddings(&self) -> Result<Vec<String>, SazidError> {
    let embeddings = self.db.list_embeddings().await?;
    Ok(embeddings.iter().map(|e| e.category.to_string()).collect::<Vec<String>>())
  }

  pub async fn list_embeddings_categories(&self) -> Result<Vec<String>, SazidError> {
    self.db.list_categories().await
  }

  pub async fn add_text_file_embedding(&self, category_name: &str, filepath: &str) -> Result<(), SazidError> {
    let texts: Vec<String> = vec![format!("Text File:\nFile Path: {}\n", filepath), std::fs::read_to_string(filepath)?];
    let texts = texts.iter().map(|t| t.as_str()).collect::<Vec<&str>>();
    let content = texts.join("");
    //create an md5sum of the file
    let data = TextFileEmbeddingData::new(&content, filepath);
    let vector = self.model.create_embedding_vector(&content).await?;
    let embedding = Embedding::new(vector, EmbeddingData::TextFileEmbedding(data), category_name.into());
    self.add_embedding(category_name, embedding).await
  }

  // Function to insert text and generate its embedding into the correct category table
  pub async fn add_embedding(&self, category_name: &str, embedding: Embedding) -> Result<(), SazidError> {
    let rows_changed = self.db.insert_embedding(category_name, embedding).await?;
    println!("rows_changed: {:?}", rows_changed);
    Ok(())
  }

  pub async fn add_text_embedding(&self, category_name: &str, content: &str) -> Result<(), SazidError> {
    //create an md5sum of the file
    let data = PlainTextEmbedding::new(content);
    let vector = self.model.create_embedding_vector(content).await?;
    let embedding = Embedding::new(vector, EmbeddingData::PlainTextEmbedding(data), category_name.into());
    self.add_embedding(category_name, embedding).await
  }
}
