#[cfg(test)]

mod vector_db_tests {
  use sazid::app::embeddings::vector_db::*;
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

    let vectordb = VectorDB { client, config: VectorDBConfig { optimize_threads: 4 } };

    // Ensure the extension is added
    vectordb.enable_extension().await?;
    // Ensure the test table exists; adjust dimensions as needed
    vectordb.create_category_table("test", 768).await?;

    Ok(vectordb)
  }

  // Shared cleanup function to be called after each test
  async fn cleanup_test_db(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    // Drop the sequence and table if they exist. The CASCADE option will take care of any dependent objects.
    client.batch_execute("DROP TABLE IF EXISTS items CASCADE;").await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_search_similar_texts() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let query_embedding = vec![0.1, 0.2, 0.3]; // Example embedding
    let similar_text_ids = db.search_similar_texts(&query_embedding, 5).await?;
    assert_eq!(similar_text_ids.len(), 5);
    // You would also verify that the IDs correspond to texts similar to the query_embedding
    Ok(())
  }

  #[tokio::test]
  async fn test_insert_text_and_get_by_id() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    db.enable_extension().await?;
    let text = "Example text";
    let embedding = vec![0.1, 0.2, 0.3]; // Example embedding
    db.insert_text_embedding("text", text, &embedding).await?;

    // Assuming the embedding insertion returns the ID of the inserted record,
    // or you retrieve it via another query after insertion
    let inserted_id = 1; // This ID should be retrieved after insertion
    let retrieved_text = db.get_text_by_id(inserted_id).await?;
    assert_eq!(text, retrieved_text, "Retrieved text does not match inserted text");

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
    // Corrected TOML configuration string
    let options = r#"indexing.flat.quantization.product.ratio = "x16""#;
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

    // Convert &[f64] to a string representation PostgreSQL can understand
    let vector_as_string = [1.0, 2.0, 3.0].iter().map(|val| val.to_string()).collect::<Vec<String>>().join(",");
    let query = format!(
      "SELECT id, embedding::text FROM items ORDER BY embedding <-> ARRAY[{}]::real[] LIMIT $1;",
      vector_as_string
    );

    // Execute the query with the proper limit parameter
    let rows = vectordb.client.query(&query, &[&(5i64)]).await?;

    let mut results = Vec::new();
    for row in rows {
      let _id: i64 = row.get(0);
      let embedding_text: String = row.get(1);
      // Parse the text back into a vector or an appropriate format for further use
      results.push(embedding_text);
    }
    assert_eq!(results.len(), 5);

    cleanup_test_db(&vectordb.client).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_create_vector_table() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    vectordb.client.batch_execute("DROP TABLE IF EXISTS items CASCADE;").await?;

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
    let _progress = VectorDB::get_indexing_progress(&vectordb.client).await?;
    // Assertions based on expected indexing progress
    vectordb.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    Ok(())
  }

  // Additional tests for edge cases, error conditions, and other methods in vector_db.rs...
}