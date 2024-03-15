use super::data_models::EmbeddingModel;
use super::types::*;
use crate::action::Action;
use crate::app::errors::SazidError;
use crate::app::session_config::SessionConfig;
use crate::cli::Cli;
use async_openai::types::ChatCompletionRequestMessage;
use dialoguer;
use diesel::{prelude::*, sql_query};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use pgvector::{Vector, VectorExpressionMethods};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Default, Debug)]
pub struct DataManager {
  pub action_tx: Option<UnboundedSender<Action>>,
  pub model: EmbeddingModel,
  pub db_url: String,
}

impl DataManager {
  pub async fn run_cli(
    &mut self,
    args: Cli,
    db_url: &str,
  ) -> Result<Option<String>, SazidError> {
    println!("args: {:#?}", args);
    Ok(match args {
      Cli { list_embeddings: true, .. } => {
        // let categories = self.list_embeddings_categories().await?;
        let embeddings = get_all_embeddings(db_url).await?;

        if embeddings.is_empty() {
          Some("No embeddings found".to_string())
        } else {
          Some(
            embeddings
              .into_iter()
              .map(|(fe, vec_ep)| {
                format!("{} -- {} pages", fe.filepath, vec_ep.len())
              })
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
        let embeddings =
          search_all_embeddings(db_url, &self.model, &text).await?;
        if embeddings.is_empty() {
          Some("No embeddings found".to_string())
        } else {
          Some(
            embeddings
              .into_iter()
              .map(|e| format!("{}", e))
              .collect::<Vec<String>>()
              .join("\n"),
          )
        }
      },
      Cli { parse_source_embeddings: Some(_), .. } => {
        // self.drop_all_embeddings_tables().await?;
        // self.parse_source_file_embeddings().await?;
        Some("parse_source_embeddings".to_string())
      },
      Cli { add_text_file_embeddings: Some(filepath), .. } => {
        // read the file at filepath
        match add_textfile_embedding(db_url, &self.model, &filepath).await {
          Ok(_) => Some(format!("Added embedding for file at {}", filepath)),
          Err(e) => Some(format!(
            "Error adding embedding for file at {}: {}",
            filepath, e
          )),
        }
      },
      Cli { add_text_embeddings: Some(_text), .. } => {
        Some("deprecated".to_string())
      },
      _ => None,
    })
  }

  pub async fn new(
    model: EmbeddingModel,
    db_url: &str,
  ) -> Result<Self, SazidError> {
    Ok(DataManager { action_tx: None, model, db_url: db_url.to_string() })
  }
}

pub async fn search_all_embeddings(
  db_url: &str,
  model: &EmbeddingModel,
  text: &str,
) -> Result<Vec<EmbeddingPage>, SazidError> {
  // create a vector of text, and then do a search for a similar vector
  let vector = model.create_embedding_vector(text).await?;
  get_similar_embeddings(db_url, vector, 10).await
}

pub async fn establish_connection(database_url: &str) -> AsyncPgConnection {
  AsyncPgConnection::establish(database_url).await.unwrap()
}

/// #
/// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// use diesel::prelude::{RunQueryDsl, Connection};
/// # let database_url = database_url();
/// let mut conn = AsyncConnectionWrapper::<DbConnection>::establish(&database_url)?;
///
/// let all_users = users::table.load::<(i32, String)>(&mut conn)?;
/// # assert_eq!(all_users.len(), 0);
pub async fn add_session(
  db_url: &str,
  config: SessionConfig,
) -> Result<QueryableSession, SazidError> {
  let conn = &mut establish_connection(db_url).await;
  use super::schema::sessions;
  let config = diesel_json::Json::new(config);
  let session = diesel::insert_into(sessions::table)
    // .values((dsl::model.eq(model.to_string()), dsl::rag.eq(rag)))
    .values(sessions::config.eq(&config))
    .returning(QueryableSession::as_returning())
    .get_result(conn)
    .await?;
  Ok(session)
}

pub async fn load_session(
  db_url: &str,
  session_id: i64,
) -> Result<QueryableSession, SazidError> {
  use super::schema::sessions::dsl::*;
  let conn = &mut establish_connection(db_url).await;
  let session = sessions
    .find(session_id)
    .select(QueryableSession::as_select())
    .first(conn)
    .await?;
  Ok(session)
}

pub async fn add_message_embedding(
  db_url: &str,
  session_id: i64,
  message_id: i64,
  model: EmbeddingModel,
  data: ChatCompletionRequestMessage,
) -> Result<i64, SazidError> {
  use super::schema::messages;

  let conn = &mut establish_connection(db_url).await;
  let data = diesel_json::Json::new(data);
  let data_json = serde_json::json!(data).to_string();
  let embedding = model.create_embedding_vector(&data_json.to_string()).await?;
  let message_id = diesel::insert_into(messages::table)
    .values((
      messages::id.eq(message_id),
      messages::session_id.eq(session_id),
      messages::data.eq(&data),
      messages::embedding.eq(embedding),
    ))
    .returning(messages::id)
    .get_result(conn)
    .await?;
  Ok(message_id)
}

pub async fn get_all_embeddings_by_session(
  db_url: &str,
  session_id: i64,
) -> Result<Vec<ChatCompletionRequestMessage>, SazidError> {
  use super::schema::messages;
  let conn = &mut establish_connection(db_url).await;
  let messages = messages::table
    .select((messages::id, messages::data))
    .filter(messages::session_id.eq(session_id))
    .load::<(i64, diesel_json::Json<ChatCompletionRequestMessage>)>(conn)
    .await?
    .into_iter()
    .map(|(_, m)| m.0)
    .collect::<Vec<ChatCompletionRequestMessage>>();
  Ok(messages)
}

pub async fn search_message_embeddings_by_session(
  db_url: &str,
  session_id: i64,
  model: &EmbeddingModel,
  text: &str,
  count: i64,
) -> Result<Vec<ChatCompletionRequestMessage>, SazidError> {
  use super::schema::messages;
  let conn = &mut establish_connection(db_url).await;
  let search_vector = model.create_embedding_vector(text).await?;
  let messages = messages::table
    .select((messages::id, messages::data))
    .filter(messages::session_id.eq(session_id))
    .order(messages::embedding.cosine_distance(&search_vector))
    .limit(count)
    .load::<(i64, diesel_json::Json<ChatCompletionRequestMessage>)>(conn)
    .await?
    .into_iter()
    .map(|(_, m)| m.0)
    .collect::<Vec<ChatCompletionRequestMessage>>();
  Ok(messages)
}

pub async fn add_embedding(
  db_url: &str,
  embedding: &InsertableFileEmbedding,
  pages: Vec<&InsertablePage>,
) -> Result<i64, SazidError> {
  use super::schema::embedding_pages;
  use super::schema::file_embeddings;
  let conn = &mut establish_connection(db_url).await;
  let embedding_id = diesel::insert_into(file_embeddings::table)
    .values(embedding)
    .on_conflict(file_embeddings::dsl::checksum)
    .do_update()
    .set(embedding)
    .returning(file_embeddings::id)
    .get_result(conn)
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
      .execute(conn)
      .await?;
  }
  Ok(embedding_id)
}

pub async fn get_all_embeddings(
  db_url: &str,
) -> Result<Vec<(FileEmbedding, Vec<EmbeddingPage>)>, SazidError> {
  use super::schema::file_embeddings::dsl::file_embeddings;
  let conn = &mut establish_connection(db_url).await;
  let all_files =
    file_embeddings.select(FileEmbedding::as_select()).load(conn).await?;

  let pages = EmbeddingPage::belonging_to(&all_files)
    .select(EmbeddingPage::as_select())
    .load(conn)
    .await?;

  Ok(
    pages
      .grouped_by(&all_files)
      .into_iter()
      .zip(all_files)
      .map(|(pages, file)| (file, pages.into_iter().collect()))
      .collect::<Vec<(FileEmbedding, Vec<EmbeddingPage>)>>(),
  )
}

pub async fn get_similar_embeddings(
  db_url: &str,
  vector: Vector,
  limit: i64,
) -> Result<Vec<EmbeddingPage>, SazidError> {
  use super::schema::embedding_pages::dsl::*;
  let conn = &mut establish_connection(db_url).await;
  let query = embedding_pages
    .select(EmbeddingPage::as_select())
    .order(embedding.cosine_distance(&vector))
    .limit(limit);
  let embeddings = query.load::<EmbeddingPage>(conn).await?;
  Ok(embeddings)
}

pub async fn add_embedding_tag(
  db_url: &str,
  tag_name: &str,
) -> Result<usize, SazidError> {
  use super::schema::tags::dsl::*;
  let conn = &mut establish_connection(db_url).await;
  Ok(diesel::insert_into(tags).values(tag.eq(tag_name)).execute(conn).await?)
}

pub async fn add_textfile_embedding(
  db_url: &str,
  model: &EmbeddingModel,
  filepath: &str,
) -> Result<i64, SazidError> {
  let content = std::fs::read_to_string(filepath)?;
  let checksum = blake3::hash(content.as_bytes()).to_hex().to_string();
  let vector_content = [filepath.to_string(), content.to_string()].join("\n");
  let embedding = model.create_embedding_vector(&vector_content).await?;
  let new_embedding = InsertableFileEmbedding {
    filepath: filepath.to_string(),
    checksum: checksum.clone(),
  };
  let new_page =
    InsertablePage { content, page_number: 0, checksum, embedding };
  add_embedding(db_url, &new_embedding, vec![&new_page]).await
}
// Method to retrieve indexing progress information
pub async fn get_indexing_progress(
  db_url: &str,
) -> Result<Vec<PgVectorIndexInfo>, SazidError> {
  let conn = &mut establish_connection(db_url).await;
  let progress_info = sql_query("SELECT * FROM pg_vector_index_info;")
    .load::<PgVectorIndexInfo>(conn)
    .await?;
  Ok(progress_info)
}
