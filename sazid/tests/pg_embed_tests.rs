#[cfg(test)]

mod pg_embed_tests {
  use diesel::prelude::*;
  use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};

  use pg_embed::pg_enums::PgAuthMethod;
  use pg_embed::pg_errors::PgEmbedError;
  use pg_embed::pg_fetch::{PgFetchSettings, PG_V15};
  use pg_embed::postgres::{PgEmbed, PgSettings};
  use pgvector::{Vector, VectorExpressionMethods};
  use std::path::PathBuf;
  use std::time::Duration;

  pub async fn setup(
    port: u16,
    database_dir: PathBuf,
    persistent: bool,
    migration_dir: Option<PathBuf>,
  ) -> Result<PgEmbed, PgEmbedError> {
    let pg_settings = PgSettings {
      database_dir,
      port,
      user: "sazid".to_string(),
      password: "alksjdlaksjdlka".to_string(),
      auth_method: PgAuthMethod::MD5,
      persistent,
      timeout: Some(Duration::from_secs(10)),
      migration_dir,
    };
    let fetch_settings =
      PgFetchSettings { version: PG_V15, ..Default::default() };
    let mut pg = PgEmbed::new(pg_settings, fetch_settings).await?;
    pg.setup().await?;
    Ok(pg)
  }

  table! {
      use diesel::sql_types::*;
       use pgvector::sql_types::*;

      plaintexts (id) {
          id -> BigInt,
          content -> Text,
          embedding -> Nullable<Vector>,
      }
  }

  #[derive(Queryable, Selectable)]
  #[diesel(table_name = plaintexts)]
  #[allow(dead_code)]
  pub struct PlainTextEmbedding {
    id: i64,
    content: String,
    embedding: Option<pgvector::Vector>,
  }

  #[derive(Insertable)]
  #[diesel(table_name = plaintexts)]
  pub struct NewPlainTextEmbedding {
    pub content: String,
    pub embedding: Option<pgvector::Vector>,
  }

  pub async fn connect_to_db(
    db_url: &str,
  ) -> anyhow::Result<AsyncPgConnection> {
    let mut conn = AsyncPgConnection::establish(db_url).await.unwrap();

    diesel::sql_query("CREATE EXTENSION IF NOT EXISTS vector")
      .execute(&mut conn)
      .await
      .unwrap();
    diesel::sql_query("DROP TABLE IF EXISTS plaintexts")
      .execute(&mut conn)
      .await
      .unwrap();
    diesel::sql_query("CREATE TABLE plaintexts (id BigSerial PRIMARY KEY, content TEXT, embedding vector(3))")
      .execute(&mut conn)
      .await.unwrap();

    Ok(conn)
  }

  #[tokio::test]
  async fn test_pg_embed() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir::TempDir::new("pg_embed_test")?;
    let migrations_dir = Some(PathBuf::from("../migrations"));
    // Create a new instance
    let mut pg =
      setup(5432, tempdir.into_path(), false, migrations_dir).await?;

    // Download, unpack, create password file and database cluster
    pg.setup().await?;

    // start postgresql database
    pg.start_db().await?;

    // create a new database
    // to enable migrations view the [Usage] section for details
    pg.create_database("database_name").await?;

    // drop a database
    // to enable migrations view [Usage] for details
    // pg.drop_database("database_name").await?;

    // check database existence
    // to enable migrations view [Usage] for details
    println!("Database exists: {}", pg.database_exists("database_name").await?);

    // run migration sql scripts
    // to enable migrations view [Usage] for details
    pg.migrate("database_name").await?;

    let pg_uri: &str = &pg.db_uri;
    println!("Postgres URI: {}", pg_uri);
    let pg_db_uri: String = pg.full_db_uri("database_name");
    println!("Postgres Database URI: {}", pg_db_uri);

    // create an async connection
    let mut conn = connect_to_db(pg_db_uri.as_str()).await.unwrap();

    let new_items = vec![
      NewPlainTextEmbedding {
        content: "hello world".to_string(),
        embedding: Some(Vector::from(vec![1.0, 1.0, 1.0])),
      },
      NewPlainTextEmbedding {
        content: "hello world".to_string(),
        embedding: Some(Vector::from(vec![1.0, 2.0, 1.0])),
      },
      NewPlainTextEmbedding {
        content: "hello world".to_string(),
        embedding: Some(Vector::from(vec![2.0, 1.0, 1.0])),
      },
      NewPlainTextEmbedding {
        content: "hello world".to_string(),
        embedding: None,
      },
    ];

    diesel::insert_into(plaintexts::table)
      .values(&new_items)
      // .returning(PlainTextEmbedding::as_returning())
      .get_result::<PlainTextEmbedding>(&mut conn).await?;

    // use ordinary diesel query dsl to construct your query
    let one = plaintexts::table
      .filter(plaintexts::id.eq(1))
      .select(PlainTextEmbedding::as_select())
      .load::<PlainTextEmbedding>(&mut conn)
      .await?;

    assert_eq!(1, one.len());

    let all = plaintexts::table.load::<PlainTextEmbedding>(&mut conn).await?;
    assert_eq!(4, all.len());

    let neighbors = plaintexts::table
      .order(
        plaintexts::embedding
          .cosine_distance(Vector::from(vec![1.0, 1.0, 1.0])),
      )
      .limit(5)
      .load::<PlainTextEmbedding>(&mut conn)
      .await?;

    assert_eq!(
      vec![1, 2, 3, 4],
      neighbors.iter().map(|v| v.id).collect::<Vec<i64>>()
    );

    // This will run the necessary migrations.
    // embedded_migrations::run(&connection);

    // pub const MIGRATIONS: EmbeddedMigrations =
    //   embed_migrations!("../migrations");
    // By default the output is thrown out. If you want to redirect it to stdout, you
    // should call embedded_migrations::run_with_output.
    // connection.run_pending_migrations(MIGRATIONS)?;
    // stop postgresql database
    pg.stop_db().await?;
    // get the base postgresql uri
    // `postgres://{username}:{password}@localhost:{port}`
    // get a postgresql database uri
    // `postgres://{username}:{password}@localhost:{port}/{specified_database_name}`

    panic!();
    Ok(())
  }
}
