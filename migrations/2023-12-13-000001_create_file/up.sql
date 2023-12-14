CREATE TABLE IF NOT EXISTS embeddings (
  id bigserial PRIMARY KEY,
  filepath TEXT,
  checksum TEXT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS tags (
  id bigserial PRIMARY KEY,
  tag TEXT NOT NULL UNIQUE,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS embedding_tags (
  embedding_id bigint REFERENCES embeddings(id) ON DELETE CASCADE,
  tag_id bigint REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (embedding_id, tag_id)
)
