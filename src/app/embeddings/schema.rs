// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    embedding_tags (embedding_id, tag_id) {
        embedding_id -> Int8,
        tag_id -> Int8,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    embeddings (id) {
        id -> Int8,
        filepath -> Nullable<Text>,
        checksum -> Text,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    pages (id) {
        id -> Int8,
        content -> Text,
        embedding -> Vector,
        checksum -> Text,
        page_number -> Int4,
        updated_at -> Timestamptz,
        embedding_id -> Int8,
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

diesel::joinable!(embedding_tags -> embeddings (embedding_id));
diesel::joinable!(embedding_tags -> tags (tag_id));
diesel::joinable!(pages -> embeddings (embedding_id));

diesel::allow_tables_to_appear_in_same_query!(
    embedding_tags,
    embeddings,
    pages,
    tags,
);
