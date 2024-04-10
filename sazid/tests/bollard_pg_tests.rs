#[cfg(test)]

mod pg_embed_tests {
  use diesel::prelude::*;
  use diesel_async::async_connection_wrapper::AsyncConnectionWrapper;
  use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
  use sazid::utils::{initialize_logging, initialize_panic_handler};

  use bollard::container::{Config, CreateContainerOptions};
  use bollard::image::CreateImageOptions;
  use bollard::models::HostConfig;
  use bollard::service::PortBinding;
  use bollard::Docker;
  use diesel_migrations::{embed_migrations, EmbeddedMigrations};
  use futures_util::StreamExt;
  use pgvector::{Vector, VectorExpressionMethods};
  use std::collections::HashMap;
  use std::env::consts;
  use std::path::PathBuf;

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

  async fn run_pgvector_container(
    port: u16,
    username: &str,
    password: &str,
    db_name: &str,
  ) -> Result<String, Box<dyn std::error::Error>> {
    let container_name = "sazid-db";
    let docker = Docker::connect_with_local_defaults().unwrap();

    // Get the platform information from the host system
    let os = match consts::OS {
      "macos" => "linux",
      _ => consts::OS,
    };

    let platform = format!("{}/{}", os, consts::ARCH);

    println!("Platform: {} {}", platform, consts::OS);
    // list the docker containers
    let containers = docker.list_containers::<String>(None).await.unwrap();
    println!("Containers: {:#?}", containers);

    if containers.iter().any(|c| c.names.as_ref().unwrap()[0].contains(container_name)) {
      println!("Container already exists. Deleting and removing it");
      docker.stop_container(container_name, None).await.unwrap();
      docker.remove_container(container_name, None).await.unwrap();
    }

    // Pull the pgvector image
    let mut create_image_stream = docker.create_image(
      Some(CreateImageOptions {
        from_image: "ankane/pgvector:latest",
        platform: &platform,
        ..Default::default()
      }),
      None,
      None,
    );

    while let Some(result) = create_image_stream.next().await {
      match result {
        Ok(output) => log::error!("error: {:?}", output),
        Err(err) => {
          log::error!("error: {:?}", err)
        },
      }
    }

    log::error!("Image pulled successfully");

    // Create container config
    let env_vars = vec![
      format!("PG_MAJOR=16"),
      format!("POSTGRES_USER={}", username),
      format!("POSTGRES_PASSWORD={}", password),
      format!("POSTGRES_DB={}", db_name),
    ];

    let mut port_bindings = HashMap::new();
    port_bindings.insert(
      "5432/tcp".to_string(),
      Some(vec![PortBinding {
        host_ip: Some("0.0.0.0".to_string()),
        host_port: Some(port.to_string()),
      }]),
    );

    let config = Config {
      image: Some("ankane/pgvector:latest"),
      env: Some(env_vars.iter().map(|s| s.as_str()).collect()),
      host_config: Some(HostConfig { port_bindings: Some(port_bindings), ..Default::default() }),
      ..Default::default()
    };

    // Create the container
    let container = docker
      .create_container(
        Some(CreateContainerOptions { name: container_name, platform: Some(&platform) }),
        config,
      )
      .await
      .unwrap();

    // Start the container
    docker.start_container::<String>(&container.id, None).await.unwrap();

    // Perform your tests here...

    // Construct the database URI
    let db_uri = format!("postgresql://{}:{}@localhost:{}/{}", username, password, port, db_name);

    Ok(db_uri)
  }

  pub async fn connect_to_db(db_url: &str) -> anyhow::Result<AsyncPgConnection> {
    // get time now
    let now = std::time::Instant::now();

    let sync_db_url = db_url.to_string();
    tokio::task::spawn_blocking(move || {
      use diesel_migrations::MigrationHarness;
      println!("Connecting to db: {}", sync_db_url);
      let mut conn = AsyncConnectionWrapper::<AsyncPgConnection>::establish(&sync_db_url).unwrap();

      diesel::RunQueryDsl::execute(
        diesel::sql_query("CREATE EXTENSION IF NOT EXISTS vector"),
        &mut conn,
      )
      .unwrap();
      diesel::RunQueryDsl::execute(diesel::sql_query("DROP TABLE IF EXISTS plaintexts"), &mut conn)
        .unwrap();
      diesel::RunQueryDsl::execute(
        diesel::sql_query(
          "CREATE TABLE plaintexts (id BigSerial PRIMARY KEY, content TEXT, embedding vector(3))",
        ),
        &mut conn,
      )
      .unwrap();

      pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");
      //
      // let conn = AsyncConnectionWrapper::from(conn);
      // // just use `run_migrations` here because that's the easiest one without additional setup
      conn.run_pending_migrations(MIGRATIONS).unwrap();
    })
    .await
    .unwrap();

    log::info!("Time taken to setup db: {:?}", now.elapsed());

    let conn = AsyncPgConnection::establish(db_url).await.unwrap();
    Ok(conn)
  }

  #[tokio::test]
  async fn test_bollard_container() -> Result<(), Box<dyn std::error::Error>> {
    initialize_logging().unwrap();
    tracing::info!("Running test_bollard_container");

    let port = 5432;
    let username = "sazid";
    let password = "db_password";
    let db_name = "embeddings";
    let uri = run_pgvector_container(port, username, password, db_name).await.unwrap();

    // run migration sql scripts
    // to enable migrations view [Usage] for details

    println!("Postgres Database URI: {}", uri);

    // create an async connection
    let mut conn = connect_to_db(uri.as_str()).await.unwrap();

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
      NewPlainTextEmbedding { content: "hello world".to_string(), embedding: None },
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
      .order(plaintexts::embedding.cosine_distance(Vector::from(vec![1.0, 1.0, 1.0])))
      .limit(5)
      .load::<PlainTextEmbedding>(&mut conn)
      .await?;

    assert_eq!(vec![1, 2, 3, 4], neighbors.iter().map(|v| v.id).collect::<Vec<i64>>());

    // This will run the necessary migrations.
    // embedded_migrations::run(&connection);

    // stop postgresql database
    // get the base postgresql uri
    // `postgres://{username}:{password}@localhost:{port}`
    // get a postgresql database uri
    // `postgres://{username}:{password}@localhost:{port}/{specified_database_name}`

    panic!();
    Ok(())
  }
}
