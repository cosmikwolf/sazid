use std::fmt;

use crate::app::errors::SazidError;

use super::schema::*;
use diesel::{prelude::*, query_dsl::methods::FindDsl};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use pgvector::Vector;

#[derive(Queryable, Selectable, Debug, Clone, Identifiable, PartialEq, Associations)]
#[diesel(belongs_to(Embedding))]
#[diesel(table_name = pages)]
pub struct Page {
  id: i64,
  embedding_id: i64,
  content: String,
  checksum: String,
  page_number: i32,
  pub embedding: Vector,
}

#[derive(Insertable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = pages)]
pub struct InsertablePage {
  pub content: String,
  pub page_number: i32,
  pub checksum: String,
  pub embedding: Vector,
}
#[derive(Queryable, Selectable, Debug, Clone, PartialEq, Identifiable, AsChangeset)]
#[diesel(table_name = embeddings)]
pub struct Embedding {
  id: i64,
  filepath: String,
  checksum: String,
}

#[derive(Insertable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = embeddings)]
pub struct InsertableEmbedding {
  filepath: String,
  checksum: String,
}
#[derive(Queryable, Selectable, Debug, Clone, PartialEq, Identifiable, AsChangeset)]
#[diesel(table_name = tags)]
pub struct Tag {
  id: i64,
  tag: String,
}

#[derive(Queryable, Selectable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = tags)]
pub struct InsertableTag {
  tag: String,
}

#[derive(Queryable, Selectable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = embedding_tags)]
pub struct EmbeddingTag {
  embedding_id: i64,
  tag_id: i64,
}

impl Embedding {
  pub async fn page_count(&self, conn: &mut AsyncPgConnection) -> Result<usize, SazidError> {
    let all_pages = FindDsl::find(pages::table, self.id).select(Embedding::as_select()).load::<Embedding>(conn).await?;
    Ok(all_pages.len())
  }
}

impl Page {
  pub async fn get_next_page(&self, conn: &mut AsyncPgConnection) -> Result<Option<Page>, SazidError> {
    let next_page = pages::table
      .filter(pages::embedding_id.eq(self.embedding_id))
      .filter(pages::page_number.gt(self.page_number))
      .order_by(pages::page_number.asc())
      .select(Page::as_select())
      .first::<Page>(conn)
      .await
      .optional()?;
    Ok(next_page)
  }

  pub async fn get_previous_page(&self, conn: &mut AsyncPgConnection) -> Result<Option<Page>, SazidError> {
    let previous_page = pages::table
      .filter(pages::embedding_id.eq(self.embedding_id))
      .filter(pages::page_number.lt(self.page_number))
      .order_by(pages::page_number.desc())
      .select(Page::as_select())
      .first::<Page>(conn)
      .await
      .optional()?;
    Ok(previous_page)
  }
}

use diesel::sql_types::{Bool, Int4, Text};
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

impl fmt::Display for Embedding {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "TextFileEmbedding(filename: {}\tchecksum: {}) ", self.filepath, self.checksum,)
  }
}

impl Iterator for Embedding {
  type Item = Embedding;
  fn next(&mut self) -> Option<Self::Item> {
    Some(self.clone())
  }
}

impl Embedding {
  pub fn content(&self) -> &str {
    match self {
      Embedding::PlainTextEmbedding(plain_text) => &plain_text.content,
      Embedding::TextFileEmbedding(text_file) => &text_file.content,
    }
  }

  pub fn variants() -> Vec<Embedding> {
    vec![
      Embedding::PlainTextEmbedding(PlainTextEmbedding { id: 0, content: "".to_string(), embedding: vec![].into() }),
      Embedding::TextFileEmbedding(Page {
        id: 0,
        content: "".to_string(),
        checksum: "".to_string(),
        embedding: vec![].into(),
      }),
    ]
  }

  fn category_name(&self) -> &str {
    match self {
      Embedding::PlainTextEmbedding(_) => "plain_text",
      Embedding::TextFileEmbedding(_) => "text_file",
    }
  }

  pub fn variant_from_category(category: &str) -> Result<Embedding, SazidError> {
    Ok(
      Self::variants()
        .iter()
        .find(|variant| variant.category_name() == category)
        .expect("No matching category")
        .clone(),
    )
  }
}
