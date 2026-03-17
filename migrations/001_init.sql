-- EMA demo migration: ensure a simple products table exists
CREATE TABLE IF NOT EXISTS Product (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT,
  price REAL
);
