CREATE TABLE repos (
  id INTEGER PRIMARY KEY NOT NULL,
  name VARCHAR NOT NULL,
  category_id INTEGER NULL DEFAULT NULL REFERENCES categories(id)
)
