CREATE TABLE file_embeddings (
  id bigserial PRIMARY KEY NOT NULL,
  filepath TEXT NOT NULL,
  checksum TEXT UNIQUE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE tags (
  id bigserial PRIMARY KEY,
  tag TEXT NOT NULL UNIQUE,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE embedding_tags (
  file_embedding_id bigint REFERENCES file_embeddings(id) ON DELETE CASCADE,
  tag_id bigint REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (file_embedding_id, tag_id)
)
