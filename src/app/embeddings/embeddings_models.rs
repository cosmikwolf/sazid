use async_openai::{
  config::OpenAIConfig,
  types::{CreateEmbeddingRequestArgs, CreateEmbeddingResponse},
};
use tokio_postgres::Row;

use crate::{
  app::{
    errors::{ParseError, SazidError},
    functions::argument_validation::count_tokens,
  },
  components::session::create_openai_client,
};

use super::types::EmbeddingVector;

#[derive(Clone)]
pub enum EmbeddingModel {
  Ada002(OpenAIConfig),
}

#[derive(Clone)]
pub struct EmbeddingModelConfig {
  pub model_string: String,
  pub token_limit: usize,
  pub embedding_suffix: String,
  pub vector_dimensions: usize,
}

impl EmbeddingModel {
  pub fn config(&self) -> EmbeddingModelConfig {
    match self {
      Self::Ada002(_) => EmbeddingModelConfig {
        model_string: "text-embedding-ada-002".to_string(),
        embedding_suffix: "ada-002".to_string(),
        token_limit: 8192,
        vector_dimensions: 768,
      },
    }
  }

  pub fn model_string(&self) -> String {
    self.config().model_string
  }
  pub fn token_limit(&self) -> usize {
    self.config().token_limit
  }

  pub fn dimensions(&self) -> usize {
    self.config().vector_dimensions
  }

  pub fn exceeds_token_limit(&self, text: &str) -> bool {
    count_tokens(text) > self.token_limit()
  }
  pub fn vec_exceeds_token_limit(&self, texts: Vec<&str>) -> bool {
    texts.iter().map(|s| count_tokens(s)).sum::<usize>() > self.token_limit()
  }

  pub async fn create_embedding_vector(&self, text: &str) -> Result<EmbeddingVector, SazidError> {
    if self.exceeds_token_limit(text) {
      Err(
        ParseError::new(&format!(
          "The total number of tokens in the input texts exceeds the limit of {} for the {} model",
          self.token_limit(),
          self.model_string()
        ))
        .into(),
      )
    } else {
      match self {
        Self::Ada002(openai_config) => {
          let client = create_openai_client(openai_config);
          let request = CreateEmbeddingRequestArgs::default().model(self.model_string()).input(text).build()?;
          let embedding_response = client.embeddings().create(request).await?;
          // embedding_response.data.iter().map(|e| e.embedding.clone()).collect::<Vec<Vec<f32>>>();
          //let embedding = embedding_response.data.first().unwrap().embedding.clone();
          Ok(embedding_response)
        },
      }
    }
    .map(EmbeddingVector::from)
  }
}
