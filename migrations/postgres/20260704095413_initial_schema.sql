-- Add migration script here-- PostgreSQL Schema
CREATE TABLE IF NOT EXISTS nodes (
    parent_hash TEXT,
    name TEXT,
    hash TEXT,
    mode INTEGER,
    size BIGINT,
    env_hash TEXT,
    PRIMARY KEY (parent_hash, name)
);

CREATE TABLE IF NOT EXISTS blobs (
    id SERIAL PRIMARY KEY,
    hash TEXT UNIQUE NOT NULL,
    content BYTEA,
    size BIGINT NOT NULL,
    mime_type TEXT
);

CREATE TABLE IF NOT EXISTS assets (
    id SERIAL PRIMARY KEY,
    uuid TEXT UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS commits (
    id SERIAL PRIMARY KEY,
    hash TEXT UNIQUE NOT NULL,
    parent_hash TEXT,
    tree_hash TEXT NOT NULL,
    author TEXT NOT NULL,
    message TEXT NOT NULL,
    ticket TEXT NOT NULL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    signature TEXT,
    env_hash TEXT
);

CREATE TABLE IF NOT EXISTS contributors (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    email TEXT UNIQUE NOT NULL,
    role TEXT NOT NULL DEFAULT 'contributor' CHECK(role IN ('maintener', 'contributor', 'tester', 'documentalist', 'ambasador'))
);

CREATE TABLE IF NOT EXISTS logs (
    id SERIAL PRIMARY KEY,
    type TEXT NOT NULL,
    state JSONB NOT NULL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS branches (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    head_id INTEGER NOT NULL REFERENCES commits(id)
);

CREATE TABLE IF NOT EXISTS manifest (
    commit_id INTEGER NOT NULL REFERENCES commits(id),
    asset_id INTEGER NOT NULL REFERENCES assets(id),
    blob_id INTEGER NOT NULL REFERENCES blobs(id),
    path TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id SERIAL PRIMARY KEY,
    sender TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS config (
    key_name TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tags (
    id SERIAL PRIMARY KEY,
    version TEXT NOT NULL UNIQUE,
    message TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);