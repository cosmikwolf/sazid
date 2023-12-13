use crate::app::errors::SazidError;
use crate::{cli::Cli, config::Config};
use diesel::prelude::*;
use diesel::sql_query;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use dotenv::dotenv;
use pgvector::{Vector, VectorExpressionMethods};

use self::embeddings_models::EmbeddingModel;
use self::schema::plaintext_embeddings::dsl::plaintext_embeddings;
use self::schema::textfile_embeddings::dsl::textfile_embeddings;
use self::types::*;
use dialoguer;

pub mod embeddings_models;
pub mod schema;
pub mod types;

pub struct EmbeddingsManager {
  client: AsyncPgConnection,
  model: EmbeddingModel,
}

impl EmbeddingsManager {
  pub async fn run(&mut self, args: Cli) -> Result<Option<String>, SazidError> {
    println!("args: {:#?}", args);
    Ok(match args {
      Cli { list_embeddings: true, .. } => {
        // let categories = self.list_embeddings_categories().await?;

        let variants = Embedding::variants();
        let mut embeddings: Vec<Embedding> = vec![];
        for variant in variants {
          let new_embeddings = self.get_embeddings_by_type(&variant).await?;
          for embedding in new_embeddings {
            embeddings.push(embedding)
          }
        }
        if embeddings.len() == 0 {
          Some("No embeddings found".to_string())
        } else {
          Some(embeddings.into_iter().map(|e| format!("{}", e)).collect::<Vec<String>>().join("\n"))
        }
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
            // self.drop_all_embeddings_tables().await?;
            Some("deleting all embeddings tables".to_string())
          },
          false => Some("cancelled".to_string()),
        }
      },
      Cli { search_embeddings: Some(text), .. } => {
        let embeddings = self.search_all_embeddings(&text).await?;
        if embeddings.len() == 0 {
          Some("No embeddings found".to_string())
        } else {
          Some(embeddings.into_iter().map(|e| format!("{}", e)).collect::<Vec<String>>().join("\n"))
        }
      },
      Cli { parse_source_embeddings: Some(_), .. } => {
        // self.drop_all_embeddings_tables().await?;
        // self.parse_source_file_embeddings().await?;
        Some("parse_source_embeddings".to_string())
      },
      Cli { add_text_file_embeddings: Some(filepath), .. } => {
        // read the file at filepath
        match self.add_textfile_embedding(&filepath).await {
          Ok(_) => Some(format!("Added embedding for file at {}", filepath)),
          Err(e) => Some(format!("Error adding embedding for file at {}: {}", filepath, e)),
        }
      },
      Cli { add_text_embeddings: Some(text), .. } => {
        self.add_plaintext_embedding(&text).await?;
        Some("load_text_embeddings".to_string())
      },
      _ => None,
    })
  }

  pub async fn search_all_embeddings(&mut self, text: &str) -> Result<Vec<Embedding>, SazidError> {
    // create a vector of text, and then do a search for a similar vector
    let vector = self.model.create_embedding_vector(text).await?;
    self.get_similar_embeddings(vector, Embedding::variants(), 10).await
  }
  pub async fn init(_config: Config, model: EmbeddingModel) -> Result<Self, SazidError> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").unwrap();
    Ok(EmbeddingsManager { client: AsyncPgConnection::establish(&database_url).await.unwrap(), model })
  }

  pub async fn add_embedding(&mut self, embedding: &InsertableEmbedding) -> Result<i64, SazidError> {
    Ok(
      match embedding {
        InsertableEmbedding::PlainText(embedding) => diesel::insert_into(plaintext_embeddings)
          .values(embedding)
          .returning(self::schema::plaintext_embeddings::id)
          .get_result(&mut self.client),

        InsertableEmbedding::TextFile(embedding) => diesel::insert_into(textfile_embeddings)
          .values(embedding)
          .on_conflict(self::schema::textfile_embeddings::dsl::checksum)
          .do_update()
          .set(embedding)
          .returning(self::schema::textfile_embeddings::id)
          .get_result(&mut self.client),
      }
      .await?,
    )
  }

  pub async fn get_embeddings_by_type(&mut self, embedding_type: &Embedding) -> Result<Vec<Embedding>, SazidError> {
    Ok(match embedding_type {
      Embedding::PlainTextEmbedding(_) => plaintext_embeddings
        .load::<PlainTextEmbedding>(&mut self.client)
        .await?
        .iter()
        .map(|e| Embedding::PlainTextEmbedding(e.clone()))
        .collect(),
      Embedding::TextFileEmbedding(_) => textfile_embeddings
        .load::<TextFileEmbedding>(&mut self.client)
        .await?
        .iter()
        .map(|e| Embedding::TextFileEmbedding(e.clone()))
        .collect(),
    })
  }
  pub async fn get_embedding_by_id(&mut self, id: i64, embedding_type: &Embedding) -> Result<Embedding, SazidError> {
    match embedding_type {
      Embedding::PlainTextEmbedding(_) => Ok(Embedding::PlainTextEmbedding(
        plaintext_embeddings.find(id).first::<PlainTextEmbedding>(&mut self.client).await?,
      )),
      Embedding::TextFileEmbedding(_) => Ok(Embedding::TextFileEmbedding(
        textfile_embeddings.find(id).first::<TextFileEmbedding>(&mut self.client).await?,
      )),
    }
  }

  pub async fn get_similar_embeddings(
    &mut self,
    vector: Vector,
    embedding_variants: Vec<Embedding>,
    limit: i64,
  ) -> Result<Vec<Embedding>, SazidError> {
    let mut similar_embeddings = Vec::new();
    for variant in embedding_variants {
      match variant {
        Embedding::TextFileEmbedding(_) => {
          let query = textfile_embeddings
            .select(self::schema::textfile_embeddings::all_columns)
            .order(self::schema::textfile_embeddings::embedding.cosine_distance(&vector))
            .limit(limit);
          let embeddings = query.load::<TextFileEmbedding>(&mut self.client).await?;
          embeddings.into_iter().for_each(|e| similar_embeddings.push(Embedding::TextFileEmbedding(e)))
        },
        Embedding::PlainTextEmbedding(_) => {
          let query = plaintext_embeddings
            .select(self::schema::plaintext_embeddings::all_columns)
            .order(self::schema::plaintext_embeddings::embedding.cosine_distance(&vector))
            .limit(limit);
          let embeddings = query.load::<PlainTextEmbedding>(&mut self.client).await?;
          embeddings.into_iter().for_each(|e| similar_embeddings.push(Embedding::PlainTextEmbedding(e)))
        },
      }
    }
    Ok(similar_embeddings)
  }

  pub async fn add_plaintext_embedding(&mut self, content: &str) -> Result<i64, SazidError> {
    let embedding = self.model.create_embedding_vector(content).await?;
    let new_embedding = InsertablePlainTextEmbedding { content: content.to_string(), embedding };
    Ok(self.add_embedding(&InsertableEmbedding::PlainText(new_embedding)).await?)
  }

  pub async fn add_textfile_embedding(&mut self, filepath: &str) -> Result<i64, SazidError> {
    let content = std::fs::read_to_string(filepath)?;
    let checksum = blake3::hash(content.as_bytes()).to_hex();
    let vector_content = vec![filepath.to_string(), content.to_string()].join("\n");
    let embedding = self.model.create_embedding_vector(&vector_content).await?;
    let new_embedding = InsertableTextFileEmbedding {
      content: content.to_string(),
      filepath: filepath.to_string(),
      checksum: checksum.to_string(),
      embedding: embedding.clone(),
    };
    Ok(self.add_embedding(&InsertableEmbedding::TextFile(new_embedding)).await?)
  }
  // Method to retrieve indexing progress information
  pub async fn get_indexing_progress(&mut self) -> Result<Vec<PgVectorIndexInfo>, SazidError> {
    let progress_info =
      sql_query("SELECT * FROM pg_vector_index_info;").load::<PgVectorIndexInfo>(&mut self.client).await?;
    Ok(progress_info)
  }
}
