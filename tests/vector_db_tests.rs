#[cfg(test)]
mod vector_db_tests {
  use sazid::app::vector_db::*;
  use tokio_postgres::{Client, NoTls};

  // Helper function to set up the test database connection
  async fn setup_test_db() -> Result<VectorDB, Box<dyn std::error::Error>> {
    let (client, connection) =
      tokio_postgres::connect("host=localhost user=postgres password=postgres-one-two-three-password", NoTls).await?;

    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("Connection error: {}", e);
      }
    });

    Ok(VectorDB { client, config: VectorDBConfig { optimize_threads: 4 } })
  }

  // Shared cleanup function to be called after each test
  async fn cleanup_test_db(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    // Drop the sequence and table if they exist. The CASCADE option will take care of any dependent objects.
    client.batch_execute("DROP SEQUENCE IF EXISTS items_id_seq;").await?;
    client.batch_execute("DROP TABLE IF EXISTS items CASCADE;").await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_enable_extension() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    vectordb.enable_extension().await?;
    // Verification step assumed
    Ok(())
  }

  #[tokio::test]
  async fn test_create_custom_index() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    vectordb.enable_extension().await?;
    vectordb.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    vectordb.create_vector_table(3).await?;
    // Create the table and sample data before index creation
    vectordb.insert_vector(&[1.0, 2.0, 3.0]).await?;

    let index_type = "l2";
    let options = r#"indexing.flat.quantization.product.ratio = \"x16\""#;
    VectorDB::create_custom_index(&vectordb.client, index_type, options).await?;
    cleanup_test_db(&vectordb.client).await?;
    // Verification step assumed
    Ok(())
  }

  #[tokio::test]
  async fn test_set_search_option() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    VectorDB::set_search_option(&vectordb.client, "vectors.k", "10").await?;
    // Verification step assumed
    cleanup_test_db(&vectordb.client).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_query_knn() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    vectordb.enable_extension().await?;
    vectordb.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    vectordb.create_vector_table(3).await?;

    for i in 0..5 {
      vectordb.insert_vector(&[i as f64, 2.0, 3.0]).await?;
    }

    let results = vectordb.query_knn(&[1.0, 2.0, 3.0], 5).await?;
    assert_eq!(results.len(), 5);
    // More detailed assertions based on expected query results
    cleanup_test_db(&vectordb.client).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_create_vector_table() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    vectordb.client.batch_execute("DROP SEQUENCE IF EXISTS items_id_seq CASCADE;").await?;
    vectordb.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    vectordb.create_vector_table(3).await?;
    // Verification step assumed
    cleanup_test_db(&vectordb.client).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_insert_vector() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    vectordb.enable_extension().await?;
    vectordb.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    vectordb.create_vector_table(3).await?;
    vectordb.insert_vector(&[1.0, 2.0, 3.0]).await?;
    vectordb.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    // Verification step assumed
    Ok(())
  }

  #[tokio::test]
  async fn test_get_indexing_progress() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    vectordb.enable_extension().await?;
    let progress = VectorDB::get_indexing_progress(&vectordb.client).await?;
    // Assertions based on expected indexing progress
    vectordb.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    Ok(())
  }

  // Additional tests for edge cases, error conditions, and other methods in vector_db.rs...
}
