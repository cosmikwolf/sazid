CREATE TABLE IF NOT EXISTS plaintext_embeddings(
  id bigserial PRIMARY KEY,
  content text NOT NULL,
  embedding vector(1536) NOT NULL
);

CREATE INDEX plaintext_cosine_index ON plaintext_embeddings USING hnsw (embedding vector_cosine_ops)
