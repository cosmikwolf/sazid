// vector_db.rs

// A Rust module for database interactions with tokio_postgres and pgvecto.rs.
use tokio_postgres::{error::SqlState, types::ToSql, Client, Error};

use super::errors::SazidError;

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
  // Methods to interact with pgvecto.rs...

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
