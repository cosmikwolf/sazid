CREATE TABLE  chat_sessions (
  id bigserial PRIMARY KEY NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);


CREATE TABLE  chat_messages (
  id bigserial PRIMARY KEY NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
