-- Add migration script here
-- 1. Table des Utilisateurs
-- Chaque utilisateur possède un rôle par défaut (ex: 'frontend', 'backend', 'designer')
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    email TEXT UNIQUE NOT NULL,
    default_role TEXT NOT NULL CHECK(default_role IN (
        -- Équipe Technique / Code
        'backend', 'frontend', 'fullstack', 'architect',
        -- Qualité & Infrastructure
        'tester', 'devops', 'security',
        -- Design & Contenu
        'designer', 'writer',
        -- Pilotage / Management
        'manager'
    ))
);

-- 2. Table des Règles de Vues par Rôle (Le cœur de ta fonctionnalité)
-- Cette table associe un rôle à un pattern SQL (ex: '%.css', 'src/backend/%') 
-- pour inclure ou exclure dynamiquement des fichiers de sa vue.
CREATE TABLE IF NOT EXISTS role_views (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    role TEXT NOT NULL,
    pattern TEXT NOT NULL, -- Exemples : '%.js', '%.php', 'assets/%', 'src/css/%'
    rule_type TEXT NOT NULL CHECK(rule_type IN ('INCLUDE', 'EXCLUDE')),
    description TEXT
);

-- 3. Table des Blobs (Contenu physique brut de tes fichiers)
CREATE TABLE IF NOT EXISTS blobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hash TEXT UNIQUE NOT NULL,
    content BLOB,
    size INTEGER NOT NULL,
    mime_type TEXT
);

-- 4. Table des Commits (Unique et propre !)
CREATE TABLE IF NOT EXISTS commits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hash TEXT UNIQUE NOT NULL,
    parent_hash TEXT,
    tree_hash TEXT NOT NULL,
    author_id INTEGER NOT NULL,
    message TEXT NOT NULL,
    insertions INTEGER NOT NULL DEFAULT 0,
    deletions INTEGER NOT NULL DEFAULT 0,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    signature TEXT,
    nix_env_hash TEXT,
    FOREIGN KEY (author_id) REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_commits_hash ON commits(hash);

-- 5. Table des Branches
CREATE TABLE IF NOT EXISTS branches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    head_commit_id INTEGER NOT NULL,
    description TEXT DEFAULT 'No description',
    expires_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (head_commit_id) REFERENCES commits(id)
);

-- 6. Table Manifeste (Le lien historique complet de ton projet)
-- Elle associe chaque commit aux fichiers présents à ce moment-là.
CREATE TABLE IF NOT EXISTS manifest (
    commit_id INTEGER NOT NULL,
    blob_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    PRIMARY KEY (commit_id, file_path),
    FOREIGN KEY (commit_id) REFERENCES commits(id),
    FOREIGN KEY (blob_id) REFERENCES blobs(id)
);

-- ==============================================================================
-- VCS CORE : L'ARBRE CRYPTOGRAPHIQUE (MERKLE TREE)
-- Indispensable pour la fonction store_tree_recursive_async dans commit.rs
-- ==============================================================================
CREATE TABLE IF NOT EXISTS nodes (
    parent_tree_hash TEXT,
    name TEXT,
    hash TEXT,
    mode INTEGER,
    size INTEGER,
    PRIMARY KEY (parent_tree_hash, name)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_nodes_parent ON nodes(parent_tree_hash);
CREATE INDEX IF NOT EXISTS idx_manifest_commit ON manifest(commit_id);
CREATE INDEX IF NOT EXISTS idx_manifest_path ON manifest(file_path);
