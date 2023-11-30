// vector_db.rs

// A Rust module for database interactions with tokio_postgres.
use tokio_postgres::{Client, Error, NoTls};

// Enable the pgvecto extension
const ENABLE_PGVECTO_EXTENSION: &str = "DROP EXTENSION IF EXISTS vectors; CREATE EXTENSION vectors;";

pub struct VectorDB {
  pub client: Client,
}

impl VectorDB {
  pub async fn enable_extension(client: &Client) -> Result<(), Error> {
    client.batch_execute(ENABLE_PGVECTO_EXTENSION).await
  }

  pub async fn new(connection_string: &str) -> Result<Self, Error> {
    let (client, connection) = tokio_postgres::connect(connection_string, NoTls).await?;
    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("connection error: {}", e);
      }
    });

    Self::enable_extension(&client).await?;

    Ok(VectorDB { client })
  }

  pub async fn insert_vector(&self, vector: &[f64]) -> Result<(), Error> {
    let stmt = self.client.prepare("INSERT INTO items (embedding) VALUES ($1::vector)").await?;
    self.client.execute(&stmt, &[&vector]).await?;
    Ok(())
  }

  pub async fn query_vectors(&self, query: &[f64], limit: i64) -> Result<Vec<Vec<f64>>, Error> {
    let stmt = self.client.prepare("SELECT embedding FROM items ORDER BY embedding <-> $1::vector LIMIT $2").await?;
    let rows = self.client.query(&stmt, &[&query, &limit]).await?;

    let mut results = Vec::new();
    for row in rows {
      results.push(row.get(0));
    }
    Ok(results)
  }
}
