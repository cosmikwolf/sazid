use super::embeddings_models::EmbeddingModel;
use super::types::*;
use crate::action::Action;
use crate::app::errors::SazidError;
use crate::trace_dbg;
use crate::{cli::Cli, config::Config};
use async_openai::types::ChatCompletionRequestMessage;
use dialoguer;
use diesel::prelude::*;
use diesel::sql_query;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use dotenv::dotenv;
use pgvector::{Vector, VectorExpressionMethods};
use tokio::sync::mpsc::UnboundedSender;

pub struct EmbeddingsManager {
  pub action_tx: Option<UnboundedSender<Action>>,
  config: Config,
  client: AsyncPgConnection,
  model: EmbeddingModel,
}

impl EmbeddingsManager {
  pub async fn run_cli(&mut self, args: Cli) -> Result<Option<String>, SazidError> {
    println!("args: {:#?}", args);
    Ok(match args {
      Cli { list_embeddings: true, .. } => {
        // let categories = self.list_embeddings_categories().await?;
        let embeddings = self.get_all_embeddings().await?;

        if embeddings.is_empty() {
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
        if embeddings.is_empty() {
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

  pub async fn new(config: Config, model: EmbeddingModel) -> Result<Self, SazidError> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").unwrap();
    Ok(EmbeddingsManager {
      action_tx: None,
      config,
      client: AsyncPgConnection::establish(&database_url).await.unwrap(),
      model,
    })
  }

  pub async fn create_session(&mut self, model: &str, prompt: &str, rag: bool) -> Result<i64, SazidError> {
    use super::schema::sessions;
    let session_id = diesel::insert_into(sessions::table)
      .values((sessions::model.eq(model), sessions::prompt.eq(prompt), sessions::rag.eq(rag)))
      .returning(sessions::id)
      .get_result(&mut self.client)
      .await?;
    Ok(session_id)
  }
  pub async fn add_message_embedding(
    &mut self,
    session_id: i64,
    message_id: Option<String>,
    data: ChatCompletionRequestMessage,
  ) -> Result<String, SazidError> {
    let message_id = match message_id {
      Some(id) => id,
      None => uuid::Uuid::new_v4().to_string(),
    };

    use super::schema::messages;
    let data = diesel_json::Json::new(data);
    trace_dbg!("embedding message data: {:#?}", data);
    let embedding = self.model.create_embedding_vector(serde_json::json!(data).as_str().unwrap()).await?;
    let message_id = diesel::insert_into(messages::table)
      .values((
        messages::id.eq(message_id),
        messages::session_id.eq(session_id),
        messages::data.eq(&data),
        messages::embedding.eq(embedding),
      ))
      .returning(messages::id)
      .get_result(&mut self.client)
      .await?;
    Ok(message_id)
  }

  pub async fn search_related_session_messages(
    &mut self,
    session_id: i64,
    text: &str,
  ) -> Result<Vec<ChatCompletionRequestMessage>, SazidError> {
    use super::schema::messages;
    let search_vector = self.model.create_embedding_vector(text).await?;
    let messages = messages::table
      .select((messages::id, messages::data))
      .filter(messages::session_id.eq(session_id))
      .order(messages::embedding.cosine_distance(&search_vector))
      .limit(10)
      .load::<(String, diesel_json::Json<ChatCompletionRequestMessage>)>(&mut self.client)
      .await?
      .into_iter()
      .map(|(_, m)| m.0)
      .collect::<Vec<ChatCompletionRequestMessage>>();
    Ok(messages)
  }

  pub async fn add_embedding(
    &mut self,
    embedding: &InsertableFileEmbedding,
    pages: Vec<&InsertablePage>,
  ) -> Result<i64, SazidError> {
    use super::schema::embedding_pages;
    use super::schema::file_embeddings;
    let embedding_id = diesel::insert_into(file_embeddings::table)
      .values(embedding)
      .on_conflict(file_embeddings::dsl::checksum)
      .do_update()
      .set(embedding)
      .returning(file_embeddings::id)
      .get_result(&mut self.client)
      .await?;
    println!("embedding_id: {}", embedding_id);

    for p in pages {
      diesel::insert_into(embedding_pages::table)
        .values((
          embedding_pages::content.eq(p.content.clone()),
          embedding_pages::page_number.eq(p.page_number),
          embedding_pages::checksum.eq(p.checksum.clone()),
          embedding_pages::file_embedding_id.eq(embedding_id),
          embedding_pages::embedding.eq(p.embedding.clone()),
        ))
        .execute(&mut self.client)
        .await?;
    }
    Ok(embedding_id)
  }

  pub async fn get_all_embeddings(&mut self) -> Result<Vec<(FileEmbedding, Vec<EmbeddingPage>)>, SazidError> {
    use super::schema::file_embeddings::dsl::file_embeddings;
    let all_files = file_embeddings.select(FileEmbedding::as_select()).load(&mut self.client).await?;

    let pages =
      EmbeddingPage::belonging_to(&all_files).select(EmbeddingPage::as_select()).load(&mut self.client).await?;

    Ok(
      pages
        .grouped_by(&all_files)
        .into_iter()
        .zip(all_files)
        .map(|(pages, file)| (file, pages.into_iter().collect()))
        .collect::<Vec<(FileEmbedding, Vec<EmbeddingPage>)>>(),
    )
  }

  pub async fn get_similar_embeddings(&mut self, vector: Vector, limit: i64) -> Result<Vec<EmbeddingPage>, SazidError> {
    use super::schema::embedding_pages::dsl::*;
    let query =
      embedding_pages.select(EmbeddingPage::as_select()).order(embedding.cosine_distance(&vector)).limit(limit);
    let embeddings = query.load::<EmbeddingPage>(&mut self.client).await?;
    Ok(embeddings)
  }

  pub async fn add_embedding_tag(&mut self, tag_name: &str) -> Result<usize, SazidError> {
    use super::schema::tags::dsl::*;
    Ok(diesel::insert_into(tags).values(tag.eq(tag_name)).execute(&mut self.client).await?)
  }

  pub async fn add_textfile_embedding(&mut self, filepath: &str) -> Result<i64, SazidError> {
    let content = std::fs::read_to_string(filepath)?;
    let checksum = blake3::hash(content.as_bytes()).to_hex().to_string();
    let vector_content = [filepath.to_string(), content.to_string()].join("\n");
    let embedding = self.model.create_embedding_vector(&vector_content).await?;
    let new_embedding = InsertableFileEmbedding { filepath: filepath.to_string(), checksum: checksum.clone() };
    let new_page = InsertablePage { content, page_number: 0, checksum, embedding };
    self.add_embedding(&new_embedding, vec![&new_page]).await
  }
  // Method to retrieve indexing progress information
  pub async fn get_indexing_progress(&mut self) -> Result<Vec<PgVectorIndexInfo>, SazidError> {
    let progress_info =
      sql_query("SELECT * FROM pg_vector_index_info;").load::<PgVectorIndexInfo>(&mut self.client).await?;
    Ok(progress_info)
  }
}
