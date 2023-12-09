CREATE TABLE IF NOT EXISTS plaintext_embeddings(
  id bigserial PRIMARY KEY,
  content TEXT NOT NULL,
  embedding VECTOR(768) NOT NULL
);
