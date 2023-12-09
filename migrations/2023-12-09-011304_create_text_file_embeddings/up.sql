CREATE TABLE IF NOT EXISTS textfile_embeddings(
  id bigserial PRIMARY KEY,
  content TEXT NOT NULL,
  filename TEXT NOT NULL,
  checksum TEXT NOT NULL,
  embedding VECTOR(768) NOT NULL
);
