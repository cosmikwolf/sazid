// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    plaintext_embeddings (id) {
        id -> Int8,
        content -> Text,
        embedding -> Vector,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    textfile_embeddings (id) {
        id -> Int8,
        content -> Text,
        filename -> Text,
        checksum -> Text,
        embedding -> Vector,
    }
}

diesel::allow_tables_to_appear_in_same_query!(plaintext_embeddings, textfile_embeddings,);
