CREATE TABLE profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    gender TEXT NOT NULL,
    gender_probability REAL NOT NULL,
    sample_size INTEGER NOT NULL,
    age INTEGER NOT NULL,
    age_group TEXT NOT NULL,
    country_id TEXT NOT NULL,
    country_probability REAL NOT NULL,
    created_at TEXT NOT NULL
);
