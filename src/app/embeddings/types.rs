use super::db_model::*;
use crate::app::errors::SazidError;
use async_openai::types::CreateEmbeddingResponse;
use pgvector::Vector;

#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
  pub embedding: Vector,
  pub category: String,
  pub data: EmbeddingData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingFileInfo {
  pub filename: String,
  pub md5sum: String,
}

impl Embedding {
  pub fn new(embedding: Vector, data: EmbeddingData, category: String) -> Self {
    Embedding { embedding, data, category }
  }

  pub fn content(&self) -> &str {
    self.data.content()
  }

  pub fn string_representation(&self) -> String {
    let vec_str = self.embedding.iter().map(|elem| elem.to_string()).collect::<Vec<String>>().join(",");
    format!("'[{}]'", vec_str)
  }

  pub fn table_name(&self) -> String {
    format!("{}_embedding", self.category)
  }
}
