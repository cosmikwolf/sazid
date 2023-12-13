CREATE TABLE IF NOT EXISTS textfile_embeddings(
  id bigserial PRIMARY KEY,
  content TEXT NOT NULL,
  filepath TEXT NOT NULL,
  checksum TEXT NOT NULL,
  embedding VECTOR(1536) NOT NULL
);

CREATE UNIQUE INDEX embedding_checksum_index ON textfile_embeddings(checksum);
CREATE INDEX textfile_cosine_index ON textfile_embeddings USING hnsw (embedding vector_cosine_ops);
