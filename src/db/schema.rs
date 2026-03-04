use rusqlite::Connection;

use crate::error::Result;

const SCHEMA_VERSION: i32 = 2;

pub fn init_schema(conn: &Connection) -> Result<()> {
    let current_version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    if current_version == 0 {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS repos (
                id TEXT PRIMARY KEY CHECK (id LIKE 'repo_%'),
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                remote_url TEXT,
                provider TEXT NOT NULL DEFAULT 'unknown',
                base_branch TEXT NOT NULL DEFAULT 'main',
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS environments (
                id TEXT PRIMARY KEY CHECK (id LIKE 'env_%'),
                repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                branch TEXT NOT NULL,
                ordinal INTEGER NOT NULL DEFAULT 0,
                auto_promote INTEGER NOT NULL DEFAULT 0,
                UNIQUE(repo_id, name)
            );

            CREATE TABLE IF NOT EXISTS topics (
                id TEXT PRIMARY KEY CHECK (id LIKE 'topic_%'),
                repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
                branch TEXT NOT NULL,
                pr_id TEXT,
                pr_url TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                ci_status TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(repo_id, branch)
            );

            CREATE TABLE IF NOT EXISTS topic_environments (
                topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
                env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
                added_at TEXT NOT NULL,
                PRIMARY KEY (topic_id, env_id)
            );

            CREATE TABLE IF NOT EXISTS rebuilds (
                id TEXT PRIMARY KEY CHECK (id LIKE 'rebuild_%'),
                env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                status TEXT NOT NULL DEFAULT 'running',
                topics_merged INTEGER NOT NULL DEFAULT 0,
                topics_conflicted INTEGER NOT NULL DEFAULT 0,
                result_sha TEXT
            );

            CREATE TABLE IF NOT EXISTS conflicts (
                id TEXT PRIMARY KEY CHECK (id LIKE 'conflict_%'),
                rebuild_id TEXT NOT NULL REFERENCES rebuilds(id) ON DELETE CASCADE,
                topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
                conflicted_with TEXT,
                resolved INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_environments_repo ON environments(repo_id);
            CREATE INDEX IF NOT EXISTS idx_topics_repo ON topics(repo_id);
            CREATE INDEX IF NOT EXISTS idx_topic_environments_env ON topic_environments(env_id);
            CREATE INDEX IF NOT EXISTS idx_topic_environments_topic ON topic_environments(topic_id);
            CREATE INDEX IF NOT EXISTS idx_rebuilds_env ON rebuilds(env_id);
            CREATE INDEX IF NOT EXISTS idx_conflicts_rebuild ON conflicts(rebuild_id);
            CREATE INDEX IF NOT EXISTS idx_conflicts_topic ON conflicts(topic_id);

            PRAGMA journal_mode = WAL;
            "#,
        )?;

        conn.pragma_update(None, "user_version", 1)?;
    }

    if current_version <= 1 {
        conn.execute_batch(
            r#"
            ALTER TABLE topics ADD COLUMN ci_url TEXT;
            ALTER TABLE topics ADD COLUMN last_ci_check TEXT;
            "#,
        )?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }

    Ok(())
}

pub fn open_db(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    init_schema(&conn)?;
    Ok(conn)
}
