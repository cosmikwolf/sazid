use crate::app::errors::SazidError;
use crate::{cli::Cli, config::Config};
use diesel::prelude::*;
use diesel::sql_query;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use dotenv::dotenv;
use pgvector::{Vector, VectorExpressionMethods};

use self::embeddings_models::EmbeddingModel;
use self::types::*;
use dialoguer;

pub mod embeddings_models;
pub mod schema;
pub mod treesitter_extraction;
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
        let embeddings = self.get_all_embeddings().await?;

        if embeddings.len() == 0 {
          Some("No embeddings found".to_string())
        } else {
          Some(
            embeddings
              .into_iter()
              .map(|(fe, vec_ep)| format!("{} -- {} pages", fe.filepath, vec_ep.len()))
              .collect::<Vec<String>>()
              .join("\n"),
          )
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
      Cli { add_text_embeddings: Some(_text), .. } => Some("deprecated".to_string()),
      _ => None,
    })
  }

  pub async fn search_all_embeddings(&mut self, text: &str) -> Result<Vec<EmbeddingPage>, SazidError> {
    // create a vector of text, and then do a search for a similar vector
    let vector = self.model.create_embedding_vector(text).await?;
    self.get_similar_embeddings(vector, 10).await
  }

  pub async fn init(_config: Config, model: EmbeddingModel) -> Result<Self, SazidError> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").unwrap();
    Ok(EmbeddingsManager { client: AsyncPgConnection::establish(&database_url).await.unwrap(), model })
  }

  pub async fn add_embedding(
    &mut self,
    embedding: &InsertableFileEmbedding,
    pages: Vec<&InsertablePage>,
  ) -> Result<i64, SazidError> {
    let embedding_id = diesel::insert_into(self::schema::file_embeddings::table)
      .values(embedding)
      .on_conflict(self::schema::file_embeddings::dsl::checksum)
      .do_update()
      .set(embedding)
      .returning(self::schema::file_embeddings::id)
      .get_result(&mut self.client)
      .await?;
    println!("embedding_id: {}", embedding_id);

    for p in pages {
      diesel::insert_into(self::schema::embedding_pages::table)
        .values((
          schema::embedding_pages::content.eq(p.content.clone()),
          schema::embedding_pages::page_number.eq(p.page_number.clone()),
          schema::embedding_pages::checksum.eq(p.checksum.clone()),
          schema::embedding_pages::file_embedding_id.eq(embedding_id),
          schema::embedding_pages::embedding.eq(p.embedding.clone()),
        ))
        .execute(&mut self.client)
        .await?;
    }
    Ok(embedding_id)
  }

  pub async fn get_all_embeddings(&mut self) -> Result<Vec<(FileEmbedding, Vec<EmbeddingPage>)>, SazidError> {
    // use schema::embedding_pages::dsl::*;
    use schema::file_embeddings::dsl::*;

    let all_files = file_embeddings.select(FileEmbedding::as_select()).load(&mut self.client).await?;

    let pages =
      EmbeddingPage::belonging_to(&all_files).select(EmbeddingPage::as_select()).load(&mut self.client).await?;

    Ok(
      pages
        .grouped_by(&all_files)
        .into_iter()
        .zip(all_files)
        .map(|(pages, file)| (file, pages.into_iter().map(|page| page).collect()))
        .collect::<Vec<(FileEmbedding, Vec<EmbeddingPage>)>>(),
    )
  }

  pub async fn get_similar_embeddings(&mut self, vector: Vector, limit: i64) -> Result<Vec<EmbeddingPage>, SazidError> {
    let query = self::schema::embedding_pages::table
      .select(EmbeddingPage::as_select())
      .order(schema::embedding_pages::embedding.cosine_distance(&vector))
      .limit(limit);
    let embeddings = query.load::<EmbeddingPage>(&mut self.client).await?;
    Ok(embeddings)
  }

  pub async fn add_embedding_tag(&mut self, tag_name: &str) -> Result<usize, SazidError> {
    Ok(diesel::insert_into(schema::tags::table).values(schema::tags::tag.eq(tag_name)).execute(&mut self.client).await?)
  }

  pub async fn add_textfile_embedding(&mut self, filepath: &str) -> Result<i64, SazidError> {
    let content = std::fs::read_to_string(filepath)?;
    let checksum = blake3::hash(content.as_bytes()).to_hex().to_string();
    let vector_content = vec![filepath.to_string(), content.to_string()].join("\n");
    let embedding = self.model.create_embedding_vector(&vector_content).await?;
    let new_embedding = InsertableFileEmbedding { filepath: filepath.to_string(), checksum: checksum.clone() };
    let new_page = InsertablePage { content, page_number: 0, checksum, embedding };
    Ok(self.add_embedding(&new_embedding, vec![&new_page]).await?)
  }
  // Method to retrieve indexing progress information
  pub async fn get_indexing_progress(&mut self) -> Result<Vec<PgVectorIndexInfo>, SazidError> {
    let progress_info =
      sql_query("SELECT * FROM pg_vector_index_info;").load::<PgVectorIndexInfo>(&mut self.client).await?;
    Ok(progress_info)
  }
}
