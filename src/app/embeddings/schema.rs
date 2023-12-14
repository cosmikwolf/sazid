// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    chat_messages (id) {
        id -> Int8,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    chat_sessions (id) {
        id -> Int8,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    embedding_pages (id) {
        id -> Int8,
        content -> Text,
        embedding -> Vector,
        checksum -> Text,
        page_number -> Int4,
        updated_at -> Timestamptz,
        file_embedding_id -> Int8,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    embedding_tags (file_embedding_id, tag_id) {
        file_embedding_id -> Int8,
        tag_id -> Int8,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    file_embeddings (id) {
        id -> Int8,
        filepath -> Text,
        checksum -> Text,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    tags (id) {
        id -> Int8,
        tag -> Text,
        updated_at -> Timestamptz,
    }
}

diesel::joinable!(embedding_pages -> file_embeddings (file_embedding_id));
diesel::joinable!(embedding_tags -> file_embeddings (file_embedding_id));
diesel::joinable!(embedding_tags -> tags (tag_id));

diesel::allow_tables_to_appear_in_same_query!(
    chat_messages,
    chat_sessions,
    embedding_pages,
    embedding_tags,
    file_embeddings,
    tags,
);
