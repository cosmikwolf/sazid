// vector_db.rs

use regex::Regex;
// A Rust module for database interactions with tokio_postgres and pgvecto.rs.
use tokio_postgres::{error::SqlState, Client, Error};

use super::{
  super::errors::SazidError,
  openai_embeddings::{create_embedding, EmbeddingModel},
};

// Struct to represent vector_db configuration
#[derive(Debug)]
pub struct VectorDBConfig {
  // other configuration fields
  pub optimize_threads: i32,
}

// Struct to represent the vector database
pub struct VectorDB {
  pub client: Client,
  pub config: VectorDBConfig,
}

impl VectorDB {
  // Function to create a dynamic table for text embeddings based on category
  pub async fn create_category_table(&self, category_name: &str, dimensions: usize) -> Result<(), Error> {
    let sanitized_category_name = Self::sanitize_category_name(category_name);
    let table_name = format!("{}_embeddings", sanitized_category_name);
    let create_table_query = format!(
      "CREATE TABLE IF NOT EXISTS {} (
        id SERIAL PRIMARY KEY,
        text TEXT NOT NULL,
        embedding vector({}) NOT NULL
      );",
      table_name, dimensions
    );
    self.client.batch_execute(&create_table_query).await
  }

  // Function to insert text and generate its embedding into the correct category table
  pub async fn insert_text_and_generate_embedding(
    &self,
    category_name: &str,
    text: &str,
    model: EmbeddingModel,
  ) -> Result<(), SazidError> {
    let embedding_response = create_embedding(text.to_string(), model).await.map_err(SazidError::from)?;
    let embeddings: Vec<Vec<f64>> = embedding_response
      .data
      .into_iter()
      .map(|d| d.embedding.iter().map(|e| *e as f64).collect::<Vec<f64>>())
      .collect();
    if let Some(embedding) = embeddings.first() {
      match self.insert_text_embedding(category_name, text, embedding).await {
        Ok(_) => Ok(()),
        Err(e) => Err(SazidError::from(e)),
      }
    } else {
      Err(SazidError::Other("Failed to generate embedding".to_string()))
    }
  }

  // Method to insert text with its embedding into the correct category table using batch_execute
  pub async fn insert_text_embedding(&self, category_name: &str, text: &str, embedding: &[f64]) -> Result<(), Error> {
    // Ensure the category table exists before attempting to insert
    self.create_category_table(category_name, embedding.len()).await?;

    let sanitized_category_name = Self::sanitize_category_name(category_name);
    let table_name = format!("{}_embeddings", sanitized_category_name);
    let embedding_as_string = embedding.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(",");

    // Concatenate full SQL command as one string
    let command = format!(
      "INSERT INTO {} (text, embedding) VALUES ('{}', ARRAY[{}]::real[]);",
      table_name, text, embedding_as_string
    );

    // Use batch_execute for SQL command without parameterized query
    self.client.batch_execute(&command).await
  }

  // Utility function to sanitize category names for table creation
  fn sanitize_category_name(category_name: &str) -> String {
    let regex = Regex::new(r"[^a-zA-Z0-9_]+").unwrap();
    regex.replace_all(category_name, "_").into_owned()
  }

  // Method to perform a cosine similarity search and return the IDs of the most similar text objects
  pub async fn search_similar_texts(&self, query_embedding: &[f64], limit: i32) -> Result<Vec<i32>, Error> {
    let embedding_as_sql_array = query_embedding.iter().map(|val| val.to_string()).collect::<Vec<String>>().join(",");
    let query = format!(
      "SELECT id FROM text_embeddings ORDER BY embedding <=> ARRAY[{}]::real[] LIMIT $1",
      embedding_as_sql_array
    );
    let rows = self.client.query(&query, &[&(limit as i64)]).await?;

    let mut ids = Vec::new();
    for row in rows {
      ids.push(row.get(0));
    }
    Ok(ids)
  }

  // Method to retrieve the original text based on its ID
  pub async fn get_text_by_id(&self, category_name: &str, text_id: i32) -> Result<String, Error> {
    let sanitized_category_name = Self::sanitize_category_name(category_name);
    let table_name = format!("{}_embeddings", sanitized_category_name);
    let query = format!("SELECT text FROM {} WHERE id = $1", table_name);

    let rows = self.client.query(&query, &[&text_id]).await?;
    let text: String = rows.get(0).expect("failed to get text").get(0);
    Ok(text)
  }

  // Create the pgvecto extension to enable vector functionality
  pub async fn enable_extension(&self) -> Result<(), Error> {
    const CREATE_EXTENSION_QUERY: &str = "CREATE EXTENSION IF NOT EXISTS vectors;";

    // Attempt to create the extension, handling any potential unique constraint errors.
    match self.client.batch_execute(CREATE_EXTENSION_QUERY).await {
      Ok(_) => Ok(()),                                                   // If successful, return Ok
      Err(e) if e.code() == Some(&SqlState::UNIQUE_VIOLATION) => Ok(()), // If the extension already exists, ignore the error
      Err(e) => Err(e),                                                  // For other errors, return the error
    }
  }

  // Method to create index with custom options
  pub async fn create_custom_index(client: &Client, index_type: &str, options: &str) -> Result<(), Error> {
    let create_index_query =
      format!("CREATE INDEX ON items USING vectors (embedding {}_ops) WITH (options = $$ {} $$);", index_type, options);
    client.batch_execute(&create_index_query).await
  }

  // Method to set search option
  pub async fn set_search_option(client: &Client, option: &str, value: &str) -> Result<(), Error> {
    let set_option_query = format!("SET {} = {};", option, value);
    client.batch_execute(&set_option_query).await
  }

  // Query vectors using a KNN search
  pub async fn query_knn(&self, vector: &[f64], limit: i64) -> Result<Vec<Vec<f64>>, Error> {
    let query = "SELECT * FROM items ORDER BY embedding <-> $1 LIMIT $2;";
    let rows = self.client.query(query, &[&vector, &limit]).await?;

    let mut results = Vec::new();
    for row in rows {
      results.push(row.get(1));
    }
    Ok(results)
  }

  // Create a table with a vector column of specified dimensions
  pub async fn create_vector_table(&self, dimensions: i32) -> Result<(), Error> {
    let query = format!("CREATE TABLE items (id bigserial PRIMARY KEY, embedding vector({}) NOT NULL);", dimensions);
    self.client.batch_execute(&query).await
  }

  // Insert a vector
  pub async fn insert_vector(&self, vector: &[f64]) -> Result<(), Error> {
    // Convert &[f64] to a string representation of a vector
    let vector_string = format!(
      "INSERT INTO items (embedding) VALUES ('[{}]');",
      vector.iter().map(ToString::to_string).collect::<Vec<String>>().join(",")
    );
    // Prepare the SQL query to insert the vector
    self.client.batch_execute(vector_string.as_str()).await?;

    Ok(())
  }
  // Calculate distance between vectors using specified operator
  pub async fn calculate_distance(
    &self,
    vector_a: &[f64],
    vector_b: &[f64],
    operator: &str,
  ) -> Result<f64, SazidError> {
    let op_query = match operator {
      "<->" => "SELECT $1::vector <-> $2::vector;",
      "<#>" => "SELECT $1::vector <#> $2::vector;",
      "<=>" => "SELECT $1::vector <=> $2::vector;",
      _ => return Err(SazidError::Other("invalid operator".to_string())),
    };

    let rows = self.client.query(op_query, &[&vector_a, &vector_b]).await.unwrap();
    let distance: f64 = rows[0].get(0);

    Ok(distance)
  }

  // Method to retrieve indexing progress information
  pub async fn get_indexing_progress(client: &Client) -> Result<Vec<IndexProgress>, Error> {
    let progress_query = "SELECT * FROM pg_vector_index_info;";
    let rows = client.query(progress_query, &[]).await?;

    let mut progress_info = Vec::new();
    for row in rows {
      let progress = IndexProgress {
        indexrelid: row.get("indexrelid"),
        indexname: row.get("indexname"),
        indexing: row.get("indexing"),
        idx_tuples: row.get("idx_tuples"),
        idx_sealed_len: row.get("idx_sealed_len"),
        idx_growing_len: row.get("idx_growing_len"),
        idx_write: row.get("idx_write"),
        idx_config: row.get("idx_config"),
      };
      progress_info.push(progress);
    }

    Ok(progress_info)
  }
}

// Struct to hold indexing progress information
pub struct IndexProgress {
  pub indexrelid: i32,
  pub indexname: String,
  pub indexing: bool,
  pub idx_tuples: i32,
  pub idx_sealed_len: i32,
  pub idx_growing_len: i32,
  pub idx_write: i32,
  pub idx_config: String,
}
