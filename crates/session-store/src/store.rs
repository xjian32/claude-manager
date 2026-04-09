use crate::error::StoreError;
use crate::models::{Session, SessionFilter, SessionUpdate, ScannedSession};
use crate::db;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

pub trait SessionStore {
    fn add_session(&mut self, session: &Session) -> Result<(), StoreError>;
    fn get_session(&self, id: &str) -> Result<Option<Session>, StoreError>;
    fn get_session_by_native_id(&self, tool: &str, session_id: &str) -> Result<Option<Session>, StoreError>;
    fn list_sessions(&self, filter: &SessionFilter) -> Result<Vec<Session>, StoreError>;
    fn update_session(&mut self, id: &str, updates: &SessionUpdate) -> Result<(), StoreError>;
    fn delete_session(&mut self, id: &str) -> Result<(), StoreError>;
    fn add_tag(&mut self, session_id: &str, tag: &str) -> Result<(), StoreError>;
    fn remove_tag(&mut self, session_id: &str, tag: &str) -> Result<(), StoreError>;
    fn get_tags(&self, session_id: &str) -> Result<Vec<String>, StoreError>;
    fn list_all_tags(&self) -> Result<Vec<String>, StoreError>;
    fn upsert_scanned(&mut self, scanned: &ScannedSession) -> Result<(), StoreError>;
}

pub struct SqliteSessionStore {
    conn: Mutex<Connection>,
}

impl SqliteSessionStore {
    pub fn new(db_path: PathBuf) -> Result<Self, StoreError> {
        let conn = db::init_db(&db_path)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        db::init_db_conn(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }
}

impl SessionStore for SqliteSessionStore {
    fn add_session(&mut self, session: &Session) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, tool, session_id, project_path, title, model, token_count, created_at, updated_at, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                session.id,
                session.tool,
                session.session_id,
                session.project_path,
                session.title,
                session.model,
                session.token_count,
                session.created_at,
                session.updated_at,
                session.metadata,
            ],
        )?;
        Ok(())
    }

    fn get_session(&self, id: &str) -> Result<Option<Session>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, tool, session_id, project_path, title, model, token_count, created_at, updated_at, metadata
             FROM sessions WHERE id = ?1"
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Session {
                id: row.get(0)?,
                tool: row.get(1)?,
                session_id: row.get(2)?,
                project_path: row.get(3)?,
                title: row.get(4)?,
                model: row.get(5)?,
                token_count: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                metadata: row.get(9)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn get_session_by_native_id(&self, tool: &str, session_id: &str) -> Result<Option<Session>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, tool, session_id, project_path, title, model, token_count, created_at, updated_at, metadata
             FROM sessions WHERE tool = ?1 AND session_id = ?2"
        )?;
        let mut rows = stmt.query(params![tool, session_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Session {
                id: row.get(0)?,
                tool: row.get(1)?,
                session_id: row.get(2)?,
                project_path: row.get(3)?,
                title: row.get(4)?,
                model: row.get(5)?,
                token_count: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                metadata: row.get(9)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn list_sessions(&self, filter: &SessionFilter) -> Result<Vec<Session>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from(
            "SELECT DISTINCT s.id, s.tool, s.session_id, s.project_path, s.title, s.model, s.token_count, s.created_at, s.updated_at, s.metadata
             FROM sessions s"
        );
        let mut conditions = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref tool) = filter.tool {
            conditions.push("s.tool = ?".to_string());
            params_vec.push(Box::new(tool.clone()));
        }
        if let Some(ref project_path) = filter.project_path {
            conditions.push("s.project_path = ?".to_string());
            params_vec.push(Box::new(project_path.clone()));
        }
        if let Some(ref query) = filter.query {
            conditions.push("(s.title LIKE ? OR s.session_id LIKE ? OR s.project_path LIKE ? OR EXISTS (SELECT 1 FROM session_tags st WHERE st.session_id = s.id AND st.tag LIKE ?))".to_string());
            let q = format!("%{}%", query);
            params_vec.push(Box::new(q.clone()));
            params_vec.push(Box::new(q.clone()));
            params_vec.push(Box::new(q.clone()));
            params_vec.push(Box::new(q));
        }
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY s.created_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
        let mut rows = stmt.query(params_refs.as_slice())?;

        let mut sessions = Vec::new();
        while let Some(row) = rows.next()? {
            sessions.push(Session {
                id: row.get(0)?,
                tool: row.get(1)?,
                session_id: row.get(2)?,
                project_path: row.get(3)?,
                title: row.get(4)?,
                model: row.get(5)?,
                token_count: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                metadata: row.get(9)?,
            });
        }

        // Filter by tags in Rust (since we need AND logic)
        if let Some(ref tags) = filter.tags {
            if !tags.is_empty() {
                sessions = sessions.into_iter().filter(|s| {
                    let session_tags = self.get_tags(&s.session_id).unwrap_or_default();
                    tags.iter().all(|t| session_tags.contains(t))
                }).collect();
            }
        }

        Ok(sessions)
    }

    fn update_session(&mut self, id: &str, updates: &SessionUpdate) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET title = COALESCE(?1, title), project_path = COALESCE(?2, project_path),
             metadata = COALESCE(?3, metadata), updated_at = ?4 WHERE id = ?5",
            params![updates.title, updates.project_path, updates.metadata, now, id],
        )?;
        Ok(())
    }

    fn delete_session(&mut self, id: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM session_tags WHERE session_id IN (SELECT session_id FROM sessions WHERE id = ?1)", params![id])?;
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn add_tag(&mut self, session_id: &str, tag: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        // Get internal id from session_id
        let id: Option<String> = conn.query_row(
            "SELECT id FROM sessions WHERE session_id = ?1", params![session_id],
            |row| row.get(0)
        ).ok();
        if let Some(id) = id {
            conn.execute(
                "INSERT OR IGNORE INTO session_tags (session_id, tag) VALUES (?1, ?2)",
                params![id, tag],
            )?;
        }
        Ok(())
    }

    fn remove_tag(&mut self, session_id: &str, tag: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let id: Option<String> = conn.query_row(
            "SELECT id FROM sessions WHERE session_id = ?1", params![session_id],
            |row| row.get(0)
        ).ok();
        if let Some(id) = id {
            conn.execute(
                "DELETE FROM session_tags WHERE session_id = ?1 AND tag = ?2",
                params![id, tag],
            )?;
        }
        Ok(())
    }

    fn get_tags(&self, session_id: &str) -> Result<Vec<String>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let id: Option<String> = conn.query_row(
            "SELECT id FROM sessions WHERE session_id = ?1", params![session_id],
            |row| row.get(0)
        ).ok();
        match id {
            Some(id) => {
                let mut stmt = conn.prepare("SELECT tag FROM session_tags WHERE session_id = ?1")?;
                let mut rows = stmt.query(params![id])?;
                let mut tags = Vec::new();
                while let Some(row) = rows.next()? {
                    tags.push(row.get(0)?);
                }
                Ok(tags)
            }
            None => Ok(Vec::new()),
        }
    }

    fn list_all_tags(&self) -> Result<Vec<String>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT tag FROM session_tags ORDER BY tag")?;
        let mut rows = stmt.query([])?;
        let mut tags = Vec::new();
        while let Some(row) = rows.next()? {
            tags.push(row.get(0)?);
        }
        Ok(tags)
    }

    fn upsert_scanned(&mut self, scanned: &ScannedSession) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Check if exists
        let exists: bool = conn.query_row(
            "SELECT 1 FROM sessions WHERE tool = ?1 AND session_id = ?2",
            params![scanned.tool, scanned.session_id],
            |_| Ok(true)
        ).unwrap_or(false);

        if exists {
            conn.execute(
                "UPDATE sessions SET project_path = ?1, model = COALESCE(?2, model),
                 metadata = COALESCE(?3, metadata), updated_at = ?4
                 WHERE tool = ?5 AND session_id = ?6",
                params![scanned.project_path, scanned.model, scanned.metadata, now, scanned.tool, scanned.session_id],
            )?;
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO sessions (id, tool, session_id, project_path, title, model, token_count, created_at, updated_at, metadata)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, NULL, ?6, ?7, ?8)",
                params![
                    id,
                    scanned.tool,
                    scanned.session_id,
                    scanned.project_path,
                    scanned.model,
                    scanned.created_at,
                    now,
                    scanned.metadata,
                ],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_session() {
        let id = uuid::Uuid::new_v4().to_string();
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let mut store = SqliteSessionStore::new(PathBuf::from(format!("/tmp/test_{}.db", id))).unwrap();
        let session = Session {
            id,
            tool: "claude".to_string(),
            session_id,
            project_path: Some("/Users/jaxx/code".to_string()),
            title: Some("Test session".to_string()),
            model: Some("opus-4-5".to_string()),
            token_count: None,
            created_at: "2026-04-09T10:00:00Z".to_string(),
            updated_at: "2026-04-09T10:00:00Z".to_string(),
            metadata: None,
        };
        store.add_session(&session).unwrap();
        let retrieved = store.get_session(&session.id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().session_id, session.session_id);
    }

    #[test]
    fn test_upsert_scanned() {
        let id = uuid::Uuid::new_v4().to_string();
        let mut store = SqliteSessionStore::new(PathBuf::from(format!("/tmp/test_{}.db", id))).unwrap();
        let scanned = ScannedSession {
            tool: "claude".to_string(),
            session_id: "da97f303-9ee8-40f6-8be5-d8feae415d78".to_string(),
            project_path: Some("/Users/jaxx/code".to_string()),
            model: Some("opus-4-5".to_string()),
            created_at: "2026-04-09T10:00:00Z".to_string(),
            metadata: None,
        };
        store.upsert_scanned(&scanned).unwrap();
        let sessions = store.list_sessions(&SessionFilter::default()).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "da97f303-9ee8-40f6-8be5-d8feae415d78");
    }
}
