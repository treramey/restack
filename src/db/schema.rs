use rusqlite::Connection;

use crate::error::Result;

const SCHEMA_VERSION: i32 = 7;

pub fn init_schema(conn: &Connection) -> Result<()> {
    let mut current_version: i32 =
        conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

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
                UNIQUE(repo_id, name)
            );

            CREATE TABLE IF NOT EXISTS topics (
                id TEXT PRIMARY KEY CHECK (id LIKE 'topic_%'),
                repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
                branch TEXT NOT NULL,
                pr_id TEXT,
                pr_url TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                branch_origin TEXT NOT NULL DEFAULT 'tracked',
                ci_status TEXT,
                ci_url TEXT,
                last_ci_check TEXT,
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
        current_version = 1;
    }

    if current_version <= 1 {
        // Columns ci_url and last_ci_check are now in initial schema
        // This migration is kept for backward compatibility with existing dbs
        conn.pragma_update(None, "user_version", 2)?;
    }

    if current_version <= 2 {
        conn.execute_batch(
            r#"
            ALTER TABLE environments ADD COLUMN ci_status TEXT;
            ALTER TABLE environments ADD COLUMN ci_url TEXT;
            ALTER TABLE environments ADD COLUMN last_ci_check TEXT;

            ALTER TABLE rebuilds ADD COLUMN ci_status TEXT;
            ALTER TABLE rebuilds ADD COLUMN ci_url TEXT;
            ALTER TABLE rebuilds ADD COLUMN ci_checked_at TEXT;
            ALTER TABLE rebuilds ADD COLUMN ci_retry_count INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE rebuilds ADD COLUMN ci_override TEXT;

            CREATE TABLE IF NOT EXISTS rebuild_topics (
                rebuild_id TEXT NOT NULL REFERENCES rebuilds(id) ON DELETE CASCADE,
                topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
                phase INTEGER NOT NULL DEFAULT 0,
                merge_order INTEGER NOT NULL,
                PRIMARY KEY (rebuild_id, topic_id)
            );

            CREATE INDEX IF NOT EXISTS idx_rebuild_topics_rebuild ON rebuild_topics(rebuild_id);
            CREATE INDEX IF NOT EXISTS idx_rebuild_topics_topic ON rebuild_topics(topic_id);
            "#,
        )?;
        conn.pragma_update(None, "user_version", 3)?;
    }

    if current_version <= 3 {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS speculative_refs (
                id TEXT PRIMARY KEY CHECK (id LIKE 'specref_%'),
                rebuild_id TEXT NOT NULL REFERENCES rebuilds(id) ON DELETE CASCADE,
                env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
                step INTEGER NOT NULL,
                topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
                sha TEXT NOT NULL,
                branch_name TEXT NOT NULL,
                ci_status TEXT,
                ci_url TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(rebuild_id, step)
            );

            CREATE INDEX IF NOT EXISTS idx_speculative_refs_rebuild ON speculative_refs(rebuild_id);
            CREATE INDEX IF NOT EXISTS idx_speculative_refs_env ON speculative_refs(env_id);
            "#,
        )?;
        conn.pragma_update(None, "user_version", 4)?;
    }

    if current_version <= 4 {
        // Drop auto_promote column if it exists (only present in DBs created before v5)
        let has_auto_promote: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('environments') WHERE name = 'auto_promote'")?
            .query_row([], |row| row.get::<_, i32>(0))
            .map(|c| c > 0)
            .unwrap_or(false);
        if has_auto_promote {
            conn.execute_batch("ALTER TABLE environments DROP COLUMN auto_promote;")?;
        }
        conn.pragma_update(None, "user_version", 5)?;
        current_version = 5;
    }

    if current_version <= 5 {
        conn.execute_batch(
            "ALTER TABLE repos ADD COLUMN refs_fingerprint TEXT;
             ALTER TABLE repos ADD COLUMN last_refreshed_at TEXT;",
        )?;
        conn.pragma_update(None, "user_version", 6)?;
        current_version = 6;
    }

    if current_version <= 6 {
        // Add branch_origin column for DBs created before it was in the initial schema
        let has_branch_origin: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('topics') WHERE name = 'branch_origin'")?
            .query_row([], |row| row.get::<_, i32>(0))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has_branch_origin {
            conn.execute_batch(
                "ALTER TABLE topics ADD COLUMN branch_origin TEXT NOT NULL DEFAULT 'tracked';",
            )?;
        }
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
