use dsync::{GenerationConfig, TableOptions};
use std::{collections::HashMap, path::PathBuf};

pub fn main() {
  let dir = env!("CARGO_MANIFEST_DIR");

  dsync::generate_files(
    PathBuf::from_iter([dir, "src/app/embeddings/schema.rs"]),
    PathBuf::from_iter([dir, "src/app/embeddings/models"]),
    GenerationConfig {
      default_table_options: TableOptions::default().use_async(),
      table_options: HashMap::new().into(),
      connection_type: "diesel_async::AsyncPgConnection".into(),
      schema_path: "crate::app::embeddings::schema::".into(),
      model_path: "crate::app::embeddings::models::".into(),
    },
  );
}
