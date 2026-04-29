CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    github_id TEXT NOT NULL UNIQUE,
    username TEXT NOT NULL,
    email TEXT,
    avatar_url TEXT,
    role TEXT NOT NULL DEFAULT 'analyst',
    is_active BOOLEAN NOT NULL DEFAULT 1,
    last_login_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

