use diesel::sql_types::*;
use serde::{Deserialize, Serialize};
use std::fmt;

use super::schema::*;
use crate::app::{errors::SazidError, session_config::SessionConfig};
use async_openai::types::ChatCompletionRequestMessage;
use diesel::{expression::ValidGrouping, prelude::*};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use diesel_json;
use pgvector::Vector;

#[derive(
  Serialize,
  Deserialize,
  Queryable,
  Selectable,
  Debug,
  Clone,
  Identifiable,
  PartialEq,
  ValidGrouping,
)]
#[diesel(table_name = sessions)]
pub struct QueryableSession {
  pub id: i64,
  pub config: diesel_json::Json<SessionConfig>,
  pub summary: Option<String>,
}

#[derive(
  Serialize,
  Queryable,
  Selectable,
  Debug,
  Clone,
  Identifiable,
  PartialEq,
  ValidGrouping,
  Associations,
)]
#[diesel(table_name = messages)]
#[diesel(belongs_to(QueryableSession, foreign_key = session_id))]
pub struct QueryableMessage {
  id: i64,
  data: diesel_json::Json<ChatCompletionRequestMessage>,
  #[serde(skip)]
  embedding: Vector,
  session_id: i64,
}

#[derive(Insertable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = sessions)]
pub struct InsertableSession {
  pub config: diesel_json::Json<SessionConfig>,
  pub summary: Option<String>,
}

#[derive(
  Serialize,
  Queryable,
  Selectable,
  Debug,
  Clone,
  Identifiable,
  PartialEq,
  Associations,
  ValidGrouping,
)]
#[diesel(belongs_to(FileEmbedding))]
#[diesel(table_name = embedding_pages)]
pub struct EmbeddingPage {
  id: i64,
  content: String,
  checksum: String,
  page_number: i32,
  #[serde(skip)]
  pub embedding: Vector,
  file_embedding_id: i64,
}

#[derive(Insertable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = embedding_pages)]
pub struct InsertablePage {
  pub content: String,
  pub page_number: i32,
  pub checksum: String,
  pub embedding: Vector,
}

#[derive(
  Serialize,
  Queryable,
  Selectable,
  Debug,
  Clone,
  PartialEq,
  Identifiable,
  AsChangeset,
  ValidGrouping,
)]
#[diesel(table_name = file_embeddings)]
pub struct FileEmbedding {
  id: i64,
  pub filepath: String,
  checksum: String,
}

#[derive(Insertable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = file_embeddings)]
pub struct InsertableFileEmbedding {
  pub filepath: String,
  pub checksum: String,
}

#[derive(Queryable, Selectable, Debug, Clone, PartialEq, Identifiable, AsChangeset)]
#[diesel(table_name = tags)]
pub struct Tag {
  id: i64,
  tag: String,
}

#[derive(Serialize)]
pub struct FileWithPages {
  #[serde(flatten)]
  pub file: FileEmbedding,
  pub pages: Vec<EmbeddingPage>,
}

#[derive(Queryable, Selectable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = tags)]
pub struct InsertableTag {
  tag: String,
}

#[derive(Debug, Clone, PartialEq, Queryable, Selectable, Associations, Identifiable)]
#[diesel(belongs_to(FileEmbedding))]
#[diesel(table_name = embedding_tags)]
#[diesel(primary_key(file_embedding_id, tag_id))]
pub struct EmbeddingTag {
  file_embedding_id: i64,
  tag_id: i64,
}

impl FileEmbedding {
  pub async fn page_count(&self, conn: &mut AsyncPgConnection) -> Result<usize, SazidError> {
    let all_pages = EmbeddingPage::belonging_to(self)
      .select(EmbeddingPage::as_select())
      .load::<EmbeddingPage>(conn)
      .await?;
    Ok(all_pages.len())
  }
}

impl EmbeddingPage {
  pub async fn get_embedding_from_page(
    &self,
    conn: &mut AsyncPgConnection,
  ) -> Result<FileEmbedding, SazidError> {
    let embedding = file_embeddings::table
      .filter(file_embeddings::id.eq(self.file_embedding_id))
      .select(FileEmbedding::as_select())
      .first::<FileEmbedding>(conn)
      .await?;
    Ok(embedding)
  }
  pub async fn get_next_page(
    &self,
    conn: &mut AsyncPgConnection,
  ) -> Result<Option<EmbeddingPage>, SazidError> {
    let next_page = embedding_pages::table
      .filter(embedding_pages::file_embedding_id.eq(self.file_embedding_id))
      .filter(embedding_pages::page_number.gt(self.page_number))
      .order_by(embedding_pages::page_number.asc())
      .select(EmbeddingPage::as_select())
      .first::<EmbeddingPage>(conn)
      .await
      .optional()?;
    Ok(next_page)
  }

  pub async fn get_previous_page(
    &self,
    conn: &mut AsyncPgConnection,
  ) -> Result<Option<EmbeddingPage>, SazidError> {
    let previous_page = embedding_pages::table
      .filter(embedding_pages::file_embedding_id.eq(self.file_embedding_id))
      .filter(embedding_pages::page_number.lt(self.page_number))
      .order_by(embedding_pages::page_number.desc())
      .select(EmbeddingPage::as_select())
      .first::<EmbeddingPage>(conn)
      .await
      .optional()?;
    Ok(previous_page)
  }
}

#[derive(QueryableByName, Debug)]
pub struct PgVectorIndexInfo {
  #[diesel(sql_type = Int4)]
  pub indexrelid: i32,
  #[diesel(sql_type = Text)]
  pub indexname: String,
  #[diesel(sql_type = Bool)]
  pub indexing: bool,
  #[diesel(sql_type = Int4)]
  pub idx_tuples: i32,
  #[diesel(sql_type = Int4)]
  pub idx_sealed_len: i32,
  #[diesel(sql_type = Int4)]
  pub idx_growing_len: i32,
  #[diesel(sql_type = Int4)]
  pub idx_write: i32,
  #[diesel(sql_type = Text)]
  pub idx_config: String,
}

// impl fmt::Display for FileWithPages {
//   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//     write!(f, "{}", self)
//   }
// }
impl fmt::Display for EmbeddingPage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "EmbeddingPage(page_number: {}\t {} bytes,  first line: {}",
      self.page_number,
      self.content.as_bytes().len(),
      self.content.lines().next().unwrap()
    )
  }
}

impl fmt::Display for FileEmbedding {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "TextFileEmbedding(filename: {}\tchecksum: {}) ", self.filepath, self.checksum,)
  }
}

impl Iterator for FileEmbedding {
  type Item = FileEmbedding;
  fn next(&mut self) -> Option<Self::Item> {
    Some(self.clone())
  }
}
