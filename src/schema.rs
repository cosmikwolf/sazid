// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "vector"))]
    pub struct Vector;
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::sql_types::Vector;

    plaintext_embeddings (id) {
        id -> Int8,
        content -> Text,
        embedding -> Vector,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::sql_types::Vector;

    textfile_embeddings (id) {
        id -> Int8,
        content -> Text,
        filename -> Text,
        checksum -> Text,
        embedding -> Vector,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    plaintext_embeddings,
    textfile_embeddings,
);
