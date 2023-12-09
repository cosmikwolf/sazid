CREATE TABLE IF NOT EXISTS textfile_embeddings(
  id bigserial PRIMARY KEY,
  text TEXT NOT NULL,
  filename TEXT NOT NULL,
  checksum TEXT NOT NULL,
  embedding vector(768) NOT NULL
);
