CREATE TABLE workspace_files(
  id bigserial PRIMARY KEY NOT NULL,
  filepath TEXT NOT NULL,
  checksum TEXT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
