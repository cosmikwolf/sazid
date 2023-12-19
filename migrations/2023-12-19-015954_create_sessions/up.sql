CREATE TABLE  sessions (
  id BIGINT PRIMARY KEY NOT NULL,
  model TEXT NOT NULL,
  prompt TEXT NOT NULL,
  rag BOOLEAN NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE  messages (
  id bigserial PRIMARY KEY NOT NULL,
  session_id BIGINT NOT NULL REFERENCES sessions(id),
  request JSONB NOT NULL DEFAULT '{}'::jsonb,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
