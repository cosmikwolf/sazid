CREATE TABLE IF NOT EXISTS pages (
  id BigSerial PRIMARY KEY NOT NULL,
  content TEXT NOT NULL,
  embedding VECTOR(1536) NOT NULL,
  checksum TEXT UNIQUE NOT NULL,
  page_number INTEGER NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  embedding_id BigSerial REFERENCES embeddings(id)
);

CREATE UNIQUE INDEX embedding_checksum_index ON pages(checksum);
CREATE INDEX embedding_cosine_index ON pages USING hnsw (embedding vector_cosine_ops);
