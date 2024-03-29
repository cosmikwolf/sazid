CREATE TABLE sessions (
  id BigSerial PRIMARY KEY NOT NULL,
  config JSONB NOT NULL,
  summary TEXT DEFAULT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE  messages (
  id BigSerial PRIMARY KEY NOT NULL,
  session_id BigSerial NOT NULL REFERENCES sessions(id),
  data JSONB NOT NULL,
  embedding VECTOR(1536) NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX messages_cosine_index ON messages USING hnsw (embedding vector_cosine_ops);
