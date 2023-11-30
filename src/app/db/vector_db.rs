// vector_db.rs

// A Rust module for database interactions with tokio_postgres.
use tokio_postgres::{Client, NoTls, Error};

// Enable the pgvecto extension
const ENABLE_PGVECTO_EXTENSION: &str = "DROP EXTENSION IF EXISTS vectors; CREATE EXTENSION vectors;";

pub struct VectorDB {
    pub(crate) client: Client,
}

impl VectorDB {
    pub async fn enable_extension(client: &Client) -> Result<(), Error> {
        client.batch_execute(ENABLE_PGVECTO_EXTENSION).await
    }

    // The rest of VectorDB implementation...
}
