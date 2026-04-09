use rusqlite::{Connection, Result};
use std::path::Path;

pub fn init_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    init_db_conn(&conn)?;
    Ok(conn)
}

pub fn init_db_conn(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            tool TEXT NOT NULL,
            session_id TEXT NOT NULL,
            project_path TEXT,
            title TEXT,
            model TEXT,
            token_count INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            metadata TEXT,
            UNIQUE(tool, session_id)
        );

        CREATE TABLE IF NOT EXISTS session_tags (
            session_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (session_id, tag),
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS tools (
            name TEXT PRIMARY KEY,
            config TEXT NOT NULL,
            enabled INTEGER DEFAULT 1,
            last_scan_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_tool ON sessions(tool);
        CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_path);
        CREATE INDEX IF NOT EXISTS idx_sessions_created ON sessions(created_at);
        CREATE INDEX IF NOT EXISTS idx_session_tags_tag ON session_tags(tag);
        CREATE INDEX IF NOT EXISTS idx_session_tags_session_id ON session_tags(session_id);
        ",
    )?;
    Ok(())
}
