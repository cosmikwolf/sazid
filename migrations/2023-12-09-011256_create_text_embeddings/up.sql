CREATE TABLE IF NOT EXISTS plaintext_embeddings(
  id bigserial PRIMARY KEY,
  content text NOT NULL,
  embedding vector(768) NOT NULL
);
