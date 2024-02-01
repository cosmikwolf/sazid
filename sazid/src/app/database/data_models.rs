use async_openai::{config::OpenAIConfig, types::CreateEmbeddingRequestArgs};
use pgvector::Vector;

use crate::{
  app::{
    errors::{ParseError, SazidError},
    functions::argument_validation::count_tokens,
  },
  components::session::create_openai_client,
};

#[derive(Debug, Clone)]
pub enum EmbeddingModel {
  Ada002(OpenAIConfig),
}

impl Default for EmbeddingModel {
  fn default() -> Self {
    Self::Ada002(OpenAIConfig::default())
  }
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
        vector_dimensions: 1536,
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

  pub async fn create_embedding_vector(&self, text: &str) -> Result<Vector, SazidError> {
    if self.exceeds_token_limit(text) {
      return Err(
        ParseError::new(&format!(
          "The total number of tokens in the input texts exceeds the limit of {} for the {} model",
          self.token_limit(),
          self.model_string()
        ))
        .into(),
      );
    }

    let vector = match self {
      Self::Ada002(openai_config) => {
        let client = create_openai_client(openai_config);
        let request = CreateEmbeddingRequestArgs::default().model(self.model_string()).input(text).build().unwrap();
        let embedding_response = client.embeddings().create(request).await.unwrap();
        // embedding_response.data.iter().map(|e| e.embedding.clone()).collect::<Vec<Vec<f32>>>();
        //let embedding = embedding_response.data.first().unwrap().embedding.clone();
        embedding_response
      },
    }
    .data
    .iter()
    .flat_map(|e| e.embedding.clone())
    .collect::<Vec<f32>>();

    Ok(vector.into())
  }
}
