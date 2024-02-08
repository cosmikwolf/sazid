CREATE TABLE symbols(
  id bigserial PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  detail TEXT,
  kind INT NOT NULL,
  start_line INT NOT NULL,
  start_character INT NOT NULL,
  end_line INT NOT NULL,
  end_character INT NOT NULL,
  file_id NOT NULL,
  FOREIGN KEY (file_id) REFERENCES workspace_files(id),
  parent_id bigserial,
  FOREIGN KEY (parent_id) REFERENCES symbols(id),
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
