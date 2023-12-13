use std::fmt;

use crate::app::errors::SazidError;

use super::schema::*;
use diesel::prelude::*;
use pgvector::Vector;

#[derive(Clone, Debug, PartialEq, Queryable, Selectable, Identifiable, AsChangeset)]
#[diesel(table_name = plaintext_embeddings)]
pub struct PlainTextEmbedding {
  pub id: i64,
  pub content: String,
  pub embedding: Vector,
}

#[derive(Queryable, Identifiable, Selectable, Debug, Clone, PartialEq)]
#[diesel(table_name = textfile_embeddings)]
pub struct TextFileEmbedding {
  id: i64,
  content: String,
  filepath: String,
  checksum: String,
  pub embedding: Vector,
}

#[derive(Insertable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = plaintext_embeddings)]
pub struct InsertablePlainTextEmbedding {
  pub content: String,
  pub embedding: Vector,
}

#[derive(Insertable, Debug, Clone, PartialEq, AsChangeset)]
#[diesel(table_name = textfile_embeddings)]
pub struct InsertableTextFileEmbedding {
  pub content: String,
  pub filepath: String,
  pub checksum: String,
  pub embedding: Vector,
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

#[derive(Debug, Clone, PartialEq)]
pub enum InsertableEmbedding {
  PlainText(InsertablePlainTextEmbedding),
  TextFile(InsertableTextFileEmbedding),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Embedding {
  PlainTextEmbedding(PlainTextEmbedding),
  TextFileEmbedding(TextFileEmbedding),
}

impl fmt::Display for Embedding {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Embedding::PlainTextEmbedding(plain_text) => {
        write!(
          f,
          "PlainTextEmbedding({} chars, {} lines) : {}",
          plain_text.content.chars().count(),
          plain_text.content.lines().count(),
          plain_text.content.lines().next().unwrap_or(&""),
        )
      },
      Embedding::TextFileEmbedding(text_file) => {
        write!(
          f,
          "TextFileEmbedding(filename: {}\tchecksum: {}): {} ",
          text_file.filepath,
          text_file.checksum,
          text_file.content.lines().next().unwrap_or(&""),
        )
      },
    }
  }
}

pub enum EmbeddingTables {
  PlainText(plaintext_embeddings::table),
  TextFile(textfile_embeddings::table),
}

pub enum EmbeddingColumn {
  PlainTextId(plaintext_embeddings::id),
  PlainTextEmbedding(plaintext_embeddings::embedding),
  TextFileId(textfile_embeddings::id),
  TextFileEmbedding(textfile_embeddings::embedding),
}

impl InsertableEmbedding {
  pub fn get_table(&self) -> EmbeddingTables {
    match self {
      InsertableEmbedding::PlainText(_) => EmbeddingTables::PlainText(plaintext_embeddings::table),
      InsertableEmbedding::TextFile(_) => EmbeddingTables::TextFile(textfile_embeddings::table),
    }
  }
  pub fn get_id_column(&self) -> EmbeddingColumn {
    match self {
      InsertableEmbedding::PlainText(_) => EmbeddingColumn::PlainTextId(plaintext_embeddings::id),
      InsertableEmbedding::TextFile(_) => EmbeddingColumn::TextFileId(textfile_embeddings::id),
    }
  }
  pub fn get_embedding_column(&self) -> EmbeddingColumn {
    match self {
      InsertableEmbedding::PlainText(_) => EmbeddingColumn::PlainTextEmbedding(plaintext_embeddings::embedding),
      InsertableEmbedding::TextFile(_) => EmbeddingColumn::TextFileEmbedding(textfile_embeddings::embedding),
    }
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
      Embedding::TextFileEmbedding(TextFileEmbedding {
        id: 0,
        content: "".to_string(),
        filepath: "".to_string(),
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
