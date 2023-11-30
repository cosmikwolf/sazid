#[cfg(test)]
mod vector_db_tests {
    use crate::app::vector_db::{VectorDB, VectorDBConfig};
    use tokio_postgres::NoTls;

    // Helper function to set up the test database connection
    async fn setup_test_db() -> Result<VectorDB, Box<dyn std::error::Error>> {
        let (client, connection) =
            tokio_postgres::connect("host=localhost user=postgres", NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        Ok(VectorDB {
            client,
            config: VectorDBConfig { optimize_threads: 4 },
        })
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

        // Create the table and sample data before index creation
        vectordb.create_vector_table(3).await?;
        vectordb.insert_vector(&[1.0, 2.0, 3.0]).await?;

        let index_type = "l2";
        let options = r#"indexing.flat.quantization.product.ratio = \"x16\""#;
        vectordb.create_custom_index(&vectordb.client, index_type, options).await?;
        // Verification step assumed
        Ok(())
    }

    #[tokio::test]
    async fn test_set_search_option() -> Result<(), Box<dyn std::error::Error>> {
        let vectordb = setup_test_db().await?;
        vectordb.set_search_option(&vectordb.client, "vectors.k", "10").await?;
        // Verification step assumed
        Ok(())
    }

    #[tokio::test]
    async fn test_query_knn() -> Result<(), Box<dyn std::error::Error>> {
        let vectordb = setup_test_db().await?;

        vectordb.create_vector_table(3).await?;
        for i in 0..5 {
            vectordb.insert_vector(&[i as f64, 2.0, 3.0]).await?;
        }

        let results = vectordb.query_knn(&[1.0, 2.0, 3.0], 5).await?;
        assert_eq!(results.len(), 5);
        // More detailed assertions based on expected query results
        Ok(())
    }

    #[tokio::test]
    async fn test_create_vector_table() -> Result<(), Box<dyn std::error::Error>> {
        let vectordb = setup_test_db().await?;
        vectordb.create_vector_table(3).await?;
        // Verification step assumed
        Ok(())
    }

    #[tokio::test]
    async fn test_insert_vector() -> Result<(), Box<dyn std::error::Error>> {
        let vectordb = setup_test_db().await?;

        vectordb.create_vector_table(3).await?;
        vectordb.insert_vector(&[1.0, 2.0, 3.0]).await?;
        // Verification step assumed
        Ok(())
    }

    #[tokio::test]
    async fn test_get_indexing_progress() -> Result<(), Box<dyn std::error::Error>> {
        let vectordb = setup_test_db().await?;

        let progress = vectordb.get_indexing_progress(&vectordb.client).await?;
        // Assertions based on expected indexing progress
        Ok(())
    }

  // Additional tests for edge cases, error conditions, and other methods in vector_db.rs...
}
