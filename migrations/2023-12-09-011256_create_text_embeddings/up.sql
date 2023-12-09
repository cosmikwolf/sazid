CREATE TABLE IF NOT EXISTS plaintext_embeddings(
  id bigserial PRIMARY KEY,
  text TEXT NOT NULL,
  embedding vector(768) NOT NULL
);
