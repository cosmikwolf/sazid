use crate::app::errors::SazidError;

use super::schema::*;
use diesel::prelude::*;
use pgvector::Vector;

#[derive(Clone, Debug, PartialEq, Queryable, Selectable)]
#[diesel(table_name = plaintext_embeddings)]
pub struct PlainTextEmbedding {
  pub id: i64,
  pub content: String,
  pub embedding: Vector,
}

#[derive(Insertable)]
#[diesel(table_name = plaintext_embeddings)]
pub struct NewPlainTextEmbedding {
  pub content: String,
  pub embedding: Vector,
}

#[derive(Queryable, Selectable, Debug, Clone, PartialEq)]
#[diesel(table_name = textfile_embeddings)]
pub struct TextFileEmbedding {
  id: i64,
  content: String,
  filename: String,
  checksum: String,
  embedding: Vector,
}

#[derive(Insertable)]
#[diesel(table_name = textfile_embeddings)]
pub struct NewTextFileEmbedding {
  pub content: String,
  pub filename: String,
  pub checksum: String,
  pub embedding: Vector,
}

pub enum EmbeddingInsertData {
  PlainText(NewPlainTextEmbedding),
  TextFile(NewTextFileEmbedding),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EmbeddingData {
  PlainTextEmbedding(PlainTextEmbedding),
  TextFileEmbedding(TextFileEmbedding),
}

impl From<NewPlainTextEmbedding> for EmbeddingInsertData {
  fn from(plaintext_embedding: NewPlainTextEmbedding) -> Self {
    EmbeddingInsertData::PlainText(plaintext_embedding)
  }
}

impl From<TextFileEmbedding> for EmbeddingInsertData {
  fn from(textfile_embedding: TextFileEmbedding) -> Self {
    EmbeddingInsertData::TextFile(textfile_embedding.into())
  }
}

impl Iterator for EmbeddingData {
  type Item = EmbeddingData;
  fn next(&mut self) -> Option<Self::Item> {
    Some(self.clone())
  }
}

impl EmbeddingData {
  pub fn content(&self) -> &str {
    match self {
      EmbeddingData::PlainTextEmbedding(plain_text) => &plain_text.content,
      EmbeddingData::TextFileEmbedding(text_file) => &text_file.content,
    }
  }

  fn variants() -> Vec<EmbeddingData> {
    vec![
      EmbeddingData::PlainTextEmbedding(PlainTextEmbedding {
        id: 0,
        content: "".to_string(),
        embedding: vec![].into(),
      }),
      EmbeddingData::TextFileEmbedding(TextFileEmbedding {
        id: 0,
        content: "".to_string(),
        filename: "".to_string(),
        checksum: "".to_string(),
        embedding: vec![].into(),
      }),
    ]
  }

  fn category_name(&self) -> &str {
    match self {
      EmbeddingData::PlainTextEmbedding(_) => "plain_text",
      EmbeddingData::TextFileEmbedding(_) => "text_file",
    }
  }

  pub fn variant_from_category(category: &str) -> Result<EmbeddingData, SazidError> {
    Ok(
      Self::variants()
        .iter()
        .find(|variant| variant.category_name() == category)
        .expect("No matching category")
        .clone(),
    )
  }
}
