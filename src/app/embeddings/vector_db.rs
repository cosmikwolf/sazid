// vector_db.rs

use futures_util::future::join_all;
use regex::Regex;
use tokio_postgres::{error::SqlState, types::ToSql, Client, Error, Row};

use super::{super::errors::SazidError, types::Embedding};
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
  pub async fn create_category_table(
    &self,
    category_name: &str,
    dimensions: usize,
    include_filename: bool,
  ) -> Result<(), Error> {
    let table_name = Self::get_table_name(category_name);
    let create_table_query = format!(
      "CREATE TABLE IF NOT EXISTS {} (
        id bigserial PRIMARY KEY,
        text TEXT NOT NULL,
        {}
        embedding vector({}) NOT NULL
      );",
      table_name,
      match include_filename {
        true => "filename TEXT NOT NULL, md5sum TEXT NOT NULL,",
        false => "",
      },
      dimensions
    );
    // println!("create_table_query: {:?}", create_table_query);
    self.client.batch_execute(&create_table_query).await
  }

  // a type that represents  Vec<Pin<Box<dyn Future<Output = Result<Vec<String>, SazidError>>>>>

  pub async fn get_embeddings_by_category(&self, categories: Vec<String>) -> Result<Vec<Embedding>, SazidError> {
    let futures_vec: Vec<_> = categories
      .iter()
      .map(|category| async move {
        let table_name = VectorDB::get_table_name(category);
        self
          .get_table_rows(&table_name)
          .await
          .into_iter()
          .flat_map(|rows| {
            rows.into_iter().map(|row| Embedding::try_from_row(row, category).unwrap()).collect::<Vec<Embedding>>()
          })
          .collect::<Vec<Embedding>>()
      })
      .collect();

    Ok(join_all(futures_vec).await.into_iter().flatten().collect())
  }

  pub async fn list_categories(&self) -> Result<Vec<String>, SazidError> {
    let query = "SELECT table_name FROM information_schema.tables WHERE table_name LIKE '%_embedding';";
    let rows = self.client.query(query, &[]).await?;
    println!("rows: {:?}", rows);
    let categories =
      rows.into_iter().map(|row| row.try_get::<&str, &str>("table_name").unwrap().to_string()).collect::<Vec<String>>();
    Ok(categories)
  }

  pub async fn list_embeddings(&self) -> Result<Vec<Embedding>, SazidError> {
    let categories = self.list_categories().await?;
    Ok(
      // categories
      //   .iter()
      //   .map(|category| {
      //     println!("category: {:?}", category);
      //     self.get_embeddings_by_category(vec![category.to_string()])
      //   })
      //   .collect::<Vec<Embedding>>(),
      vec![],
    )
  }

  pub async fn get_table_rows(&self, table_name: &str) -> Result<Vec<Row>, SazidError> {
    let query = format!("SELECT * from {};", table_name);
    Ok(self.client.query(&query, &[]).await?)
  }

  async fn list_embeddings_in_table(&self, table_name: &str) -> Result<Vec<String>, SazidError> {
    let query = format!("SELECT * from {};", table_name);
    let rows = self.client.query(&query, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect::<Vec<String>>())
  }

  // Method to insert text with its embedding into the correct category table using batch_execute
  pub async fn insert_embedding(&self, category_name: &str, embedding: Embedding) -> Result<u64, Error> {
    // Ensure the category table exists before attempting to insert
    self.create_category_table(category_name, embedding.len(), false).await?;

    let table_name = Self::get_table_name(category_name);
    println!("table_name: {:?}", table_name);
    // Concatenate full SQL command as one string
    let command = format!("INSERT INTO {} (text, embedding) VALUES ($1, $2);", table_name);
    // Create a slice of references to trait objects
    let text_params: &(dyn ToSql + Sync) = &embedding.content();
    // println!("text_params: {:?}", text_params);
    let embedding_params: &(dyn ToSql + Sync) = &embedding;
    // println!("embedding_params: {:?}", embedding_params);

    // Pass the parameters as a slice of trait objects implementing ToSql
    self.client.execute(&command, &[text_params, embedding_params]).await
  }

  // Utility function to sanitize category names for table creation
  pub fn get_table_name(category_name: &str) -> String {
    let regex = Regex::new(r"[^a-zA-Z0-9_]+").unwrap();
    let table_name = regex.replace_all(category_name, "_").into_owned();
    format!("{}_embedding", table_name)
  }

  // Method to perform a cosine similarity search and return the IDs of the most similar text objects
  pub async fn search_similar_texts(
    &self,
    category_name: &str,
    query_embedding: &[f64],
    limit: i32,
  ) -> Result<Vec<Embedding>, SazidError> {
    let table_name = Self::get_table_name(category_name);
    let embedding_as_sql_array = query_embedding.iter().map(|val| val.to_string()).collect::<Vec<String>>().join(",");
    let query = format!(
      // "SELECT * ORDER BY embedding <=> {}::vector LIMIT {}", table_name, embedding_as_sql_array
      "SELECT * FROM {} ORDER BY embedding <-> '[{}]' LIMIT {}",
      table_name, embedding_as_sql_array, limit
    );
    let rows = self.client.simple_query(&query).await?;
    let vectors = Embedding::from_simple_query_messages(&rows, category_name)?;
    Ok(vectors)
  }

  // Method to retrieve the original text based on its ID
  pub async fn get_text_by_id(&self, category_name: &str, text_id: i64) -> Result<String, Error> {
    let table_name = Self::get_table_name(category_name);
    let query = format!("SELECT text FROM {} WHERE id = $1", table_name);

    let rows = self.client.query(&query, &[&text_id]).await?;
    let text: String = rows.get(0).expect("failed to get text").get("text");

    println!("text: {:?}", text);

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
  pub async fn create_custom_index(
    client: &Client,
    category_name: &str,
    index_type: &str,
    options: &str,
  ) -> Result<(), Error> {
    let table_name = Self::get_table_name(category_name);
    let create_index_query = format!(
      "CREATE INDEX ON {} USING vectors (embedding {}_ops) WITH (options = $$ {} $$);",
      table_name, index_type, options
    );
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

  // Insert a vector
  pub async fn insert_vector(&self, category_name: &str, vector: &[f64]) -> Result<(), Error> {
    let table_name = Self::get_table_name(category_name);
    // Convert &[f64] to a string representation of a vector
    let vector_string = format!(
      "INSERT INTO {} (embedding) VALUES ('[{}]');",
      table_name,
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
