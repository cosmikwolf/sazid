#[cfg(test)]

mod vector_db_tests {
  use sazid::app::embeddings::vector_db::*;
  use sazid::app::{embeddings::types::EmbeddingVector, errors::SazidError};
  use tokio_postgres::{Client, NoTls};

  // Helper function to set up the test database connection
  async fn setup_test_db() -> Result<VectorDB, SazidError> {
    let (client, connection) =
      tokio_postgres::connect("host=localhost user=postgres password=postgres-one-two-three-password", NoTls).await?;

    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("Connection error: {}", e);
      }
    });

    let vectordb = VectorDB { client, config: VectorDBConfig { optimize_threads: 4 } };

    vectordb.enable_extension().await?;
    vectordb.create_category_table("test", 3).await?;
    Ok(vectordb)
  }

  // Shared cleanup function to be called after each test
  async fn cleanup_test_db(client: &Client, category_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Drop the sequence and table if they exist. The CASCADE option will take care of any dependent objects.
    client.batch_execute(format!("DROP TABLE IF EXISTS {}_embeddings CASCADE;", category_name).as_str()).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_search_similar_texts() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let category_name = "testttt";
    // Insert texts with embeddings
    cleanup_test_db(&db.client, category_name).await?;
    for i in 1..=5 {
      let text = format!("Example text {}", i);
      let embedding = EmbeddingVector::new(vec![1.0_f64, 2.0_f64, 3.0_f64 + i as f64 / 1000000_f64], text);
      db.insert_text_embedding(category_name, embedding).await?;
    }

    // Query for a vector that is similar to the inserted vectors
    let query_embedding = vec![1.0, 2.0, 3.0];
    let similar_text_ids = db.search_similar_texts(category_name, &query_embedding, 5).await?;

    // Since there are 5 vectors inserted, we expect to retrieve 5 similar texts
    assert_eq!(similar_text_ids.len(), 5, "Should retrieve 5 similar texts");

    Ok(())
  }

  #[tokio::test]
  async fn test_vector_insert_and_retrieve() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let table_name = "items";
    cleanup_test_db(&db.client, table_name).await?;

    let vector = EmbeddingVector::new(vec![1.0, 2.0, 3.0], "testtext".into());

    db.insert_text_embedding("items", vector.clone()).await?;

    let rows = db.client.simple_query("SELECT * FROM items_embeddings ORDER BY id DESC LIMIT 1").await?;
    println!("rows: {:?}", rows);
    let vectors = EmbeddingVector::from_simple_query_messages(&rows)?;
    println!("vectors: {:#?}", vectors);
    assert_eq!(vector, vectors[0]);
    cleanup_test_db(&db.client, table_name).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_insert_text_and_retrieve_by_id() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let table_name = "items";
    cleanup_test_db(&db.client, table_name).await?;
    let text = "Example text";
    let embedding = vec![0.1, 0.2, 0.3]; // Example embedding
    let vector = EmbeddingVector::new(embedding, text.into());
    db.insert_text_embedding("test", vector).await?;

    // Retrieve the inserted text and embedding by ID
    let text_id: i32 =
      db.client.query_one("SELECT id FROM test_embeddings ORDER BY id DESC LIMIT 1", &[]).await?.get(0);
    let retrieved_text = db.get_text_by_id("test", text_id).await?;
    assert_eq!(text, retrieved_text, "Retrieved text does not match inserted text");

    cleanup_test_db(&db.client, table_name).await?;
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
    let db = setup_test_db().await?;
    let table_name = "items";
    cleanup_test_db(&db.client, table_name).await?;
    db.enable_extension().await?;
    db.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    db.create_vector_table(3).await?;
    // Create the table and sample data before index creation
    db.insert_vector(&[1.0, 2.0, 3.0]).await?;

    let index_type = "l2";
    // Corrected TOML configuration string
    let options = r#"indexing.flat.quantization.product.ratio = "x16""#;
    VectorDB::create_custom_index(&db.client, index_type, options).await?;

    cleanup_test_db(&db.client, table_name).await?;
    // Verification step assumed
    Ok(())
  }

  #[tokio::test]
  async fn test_set_search_option() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let table_name = "items";
    VectorDB::set_search_option(&db.client, "vectors.k", "10").await?;
    // Verification step assumed
    cleanup_test_db(&db.client, table_name).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_query_knn() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let table_name = "items";
    cleanup_test_db(&db.client, table_name).await?;
    db.enable_extension().await?;
    db.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    db.create_vector_table(3).await?;

    for i in 0..5 {
      db.insert_vector(&[i as f64, 2.0, 3.0]).await?;
    }

    // Convert &[f64] to a string representation PostgreSQL can understand
    let vector_as_string = [1.0, 2.0, 3.0].iter().map(|val| val.to_string()).collect::<Vec<String>>().join(",");
    let query = format!(
      "SELECT id, embedding::text FROM items ORDER BY embedding <-> ARRAY[{}]::real[] LIMIT $1;",
      vector_as_string
    );

    // Execute the query with the proper limit parameter
    let rows = db.client.query(&query, &[&(5i64)]).await?;

    let mut results = Vec::new();
    for row in rows {
      let _id: i64 = row.get(0);
      let embedding_text: String = row.get(1);
      // Parse the text back into a vector or an appropriate format for further use
      results.push(embedding_text);
    }
    assert_eq!(results.len(), 5);

    cleanup_test_db(&db.client, table_name).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_create_vector_table() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let table_name = "items";
    cleanup_test_db(&db.client, table_name).await?;
    db.client.batch_execute("DROP TABLE IF EXISTS items CASCADE;").await?;

    db.create_vector_table(3).await?;
    // Verification step assumed
    cleanup_test_db(&db.client, table_name).await?;
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
