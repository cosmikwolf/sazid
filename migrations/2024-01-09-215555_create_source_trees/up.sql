CREATE TABLE source_trees(
  id bigserial PRIMARY KEY NOT NULL,
  filepath TEXT NOT NULL,
  syntax_tree TEXT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
