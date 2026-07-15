-- ==============================================================================
-- 1. CONFIGURATION GLOBALE
-- Indispensable pour ton outil CLI (notamment pour la fonction current_branch)
-- ==============================================================================
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- On initialise la branche par défaut pour éviter les crashs côté Rust
INSERT OR IGNORE INTO config (key, value) VALUES ('current_branch', 'main');

-- ==============================================================================
-- 2. GESTION DE PROJET (TICKETS & ÉQUIPES)
-- ==============================================================================
CREATE TABLE IF NOT EXISTS todos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT DEFAULT 'TODO' CHECK(status IN ('TODO', 'IN_PROGRESS', 'REVIEW', 'DONE')),
    reporter_id INTEGER NOT NULL,
    assignee_id INTEGER,
    closing_commit_id INTEGER, -- Permet de savoir quel commit a résolu le ticket
    due_date DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (reporter_id) REFERENCES users(id),
    FOREIGN KEY (assignee_id) REFERENCES users(id),
    FOREIGN KEY (closing_commit_id) REFERENCES commits(id)
);

CREATE TABLE IF NOT EXISTS teams (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    description TEXT
);

CREATE TABLE IF NOT EXISTS team_members (
    team_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    role TEXT DEFAULT 'member',
    PRIMARY KEY (team_id, user_id),
    FOREIGN KEY (team_id) REFERENCES teams(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Le chat est maintenant sécurisé : il pointe vers un vrai utilisateur (sender_id)
CREATE TABLE IF NOT EXISTS chat (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sender_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL,
    FOREIGN KEY (sender_id) REFERENCES users(id)
);

-- ==============================================================================
-- 3. VERSIONING ÉTENDU (RELEASES)
-- ==============================================================================
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version TEXT NOT NULL UNIQUE,
    message TEXT NOT NULL,
    target_commit_id INTEGER NOT NULL, -- Un tag doit pointer vers un commit précis
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (target_commit_id) REFERENCES commits(id)
);

-- ==============================================================================
-- 4. AUTOMATISATION & EXTENSIBILITÉ (CI/CD, HOOKS, PLUGINS)
-- ==============================================================================
CREATE TABLE IF NOT EXISTS hooks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    trigger TEXT NOT NULL,
    command TEXT NOT NULL,
    is_active BOOLEAN DEFAULT FALSE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS hook_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hook_id INTEGER NOT NULL,
    commit_hash TEXT,
    exit_code INTEGER NOT NULL,
    output TEXT,
    run_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hook_id) REFERENCES hooks(id)
);

CREATE TABLE IF NOT EXISTS plugins (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    version TEXT NOT NULL,
    is_enabled BOOLEAN DEFAULT TRUE,
    config JSON
);

-- ==============================================================================
-- 5. AUDIT (OPLOG)
-- ==============================================================================
CREATE TABLE IF NOT EXISTS oplog (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_type TEXT NOT NULL,
    view_state JSON NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);
-- ==============================================================================
-- SUPPORT EXTERNE (TICKETS)
-- ==============================================================================
CREATE TABLE IF NOT EXISTS tickets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    reporter_email TEXT,       -- L'email de l'utilisateur externe (optionnel)
    status TEXT DEFAULT 'OPEN' CHECK(status IN ('OPEN', 'RESOLVED', 'CLOSED')),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ==============================================================================
-- LIAISON DANS LES TODOS
-- ==============================================================================
-- (Si tu as déjà créé la table, il faut recréer ou faire un ALTER TABLE)
-- On ajoute un champ `ticket_id` optionnel dans tes `todos` pour lier la tâche au signalement externe.
ALTER TABLE todos ADD COLUMN ticket_id INTEGER REFERENCES tickets(id);

-- ==============================================================================
-- LIAISON DANS LES COMMITS
-- ==============================================================================
-- Ta table `commits` actuelle (dans create_views.sql) n'a pas de lien vers les todos !
ALTER TABLE commits ADD COLUMN todo_id INTEGER REFERENCES todos(id);
