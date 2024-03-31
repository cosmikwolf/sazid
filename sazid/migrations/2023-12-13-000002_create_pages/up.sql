CREATE TABLE IF NOT EXISTS embedding_pages (
  id BigSerial PRIMARY KEY NOT NULL,
  content TEXT NOT NULL,
  embedding VECTOR(1536) NOT NULL,
  checksum TEXT UNIQUE NOT NULL,
  page_number INTEGER NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  file_embedding_id BigSerial,
  FOREIGN KEY (file_embedding_id) REFERENCES file_embeddings(id)
);

CREATE UNIQUE INDEX pages_checksum_index ON embedding_pages(checksum);
CREATE INDEX pages_cosine_index ON embedding_pages USING hnsw (embedding vector_cosine_ops);
