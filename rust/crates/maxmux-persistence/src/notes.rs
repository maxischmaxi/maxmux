use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NotesError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct NotesDb {
    conn: Connection,
}

impl NotesDb {
    /// Open or create the notes database at the given path.
    /// If no path is provided, defaults to `~/.maxmux/notes.db`.
    pub fn open(path: Option<PathBuf>) -> Result<Self, NotesError> {
        let path = path.unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".maxmux")
                .join("notes.db")
        });

        // Create parent directory if it does not exist.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path)?;
        Self::initialize(conn)
    }

    /// Open an in-memory database (useful for testing).
    pub fn open_in_memory() -> Result<Self, NotesError> {
        let conn = Connection::open_in_memory()?;
        Self::initialize(conn)
    }

    fn initialize(conn: Connection) -> Result<Self, NotesError> {
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;
        Ok(Self { conn })
    }

    /// List all notes, ordered by `updated_at` descending.
    pub fn list(&self) -> Result<Vec<Note>, NotesError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, content, created_at, updated_at
             FROM notes ORDER BY updated_at DESC",
        )?;
        let notes = stmt
            .query_map([], |row| {
                Ok(Note {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(notes)
    }

    /// Get a single note by ID.
    pub fn get(&self, id: &str) -> Result<Option<Note>, NotesError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, content, created_at, updated_at
             FROM notes WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Save a note (insert or replace).
    pub fn save(&self, note: &Note) -> Result<(), NotesError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO notes (id, title, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                note.id,
                note.title,
                note.content,
                note.created_at,
                note.updated_at
            ],
        )?;
        Ok(())
    }

    /// Delete a note by ID. Returns `true` if a note was deleted.
    pub fn delete(&self, id: &str) -> Result<bool, NotesError> {
        let count = self
            .conn
            .execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    /// Search notes whose title or content contains the query string (case-insensitive).
    pub fn search(&self, query: &str) -> Result<Vec<Note>, NotesError> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, title, content, created_at, updated_at
             FROM notes
             WHERE title LIKE ?1 OR content LIKE ?1
             ORDER BY updated_at DESC",
        )?;
        let notes = stmt
            .query_map(params![pattern], |row| {
                Ok(Note {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(notes)
    }

    /// Count total notes in the database.
    pub fn count(&self) -> Result<usize, NotesError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_note(id: &str, title: &str, content: &str, created_at: u64, updated_at: u64) -> Note {
        Note {
            id: id.to_string(),
            title: title.to_string(),
            content: content.to_string(),
            created_at,
            updated_at,
        }
    }

    #[test]
    fn test_create_database_and_list_empty() {
        let db = NotesDb::open_in_memory().unwrap();
        let notes = db.list().unwrap();
        assert!(notes.is_empty());
    }

    #[test]
    fn test_save_and_retrieve_note() {
        let db = NotesDb::open_in_memory().unwrap();
        let note = make_note("n1", "Hello", "World", 1000, 1000);
        db.save(&note).unwrap();

        let retrieved = db.get("n1").unwrap().expect("note should exist");
        assert_eq!(retrieved.id, "n1");
        assert_eq!(retrieved.title, "Hello");
        assert_eq!(retrieved.content, "World");
        assert_eq!(retrieved.created_at, 1000);
        assert_eq!(retrieved.updated_at, 1000);
    }

    #[test]
    fn test_save_updates_existing_note() {
        let db = NotesDb::open_in_memory().unwrap();
        let note = make_note("n1", "Title", "Content v1", 1000, 1000);
        db.save(&note).unwrap();

        let updated = make_note("n1", "Title Updated", "Content v2", 1000, 2000);
        db.save(&updated).unwrap();

        let retrieved = db.get("n1").unwrap().expect("note should exist");
        assert_eq!(retrieved.title, "Title Updated");
        assert_eq!(retrieved.content, "Content v2");
        assert_eq!(retrieved.updated_at, 2000);
        // created_at is replaced too since INSERT OR REPLACE replaces the whole row
        assert_eq!(retrieved.created_at, 1000);

        // Should still be only one note
        assert_eq!(db.count().unwrap(), 1);
    }

    #[test]
    fn test_delete_note() {
        let db = NotesDb::open_in_memory().unwrap();
        let note = make_note("n1", "Title", "Content", 1000, 1000);
        db.save(&note).unwrap();

        let deleted = db.delete("n1").unwrap();
        assert!(deleted);

        let retrieved = db.get("n1").unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_delete_nonexistent_returns_false() {
        let db = NotesDb::open_in_memory().unwrap();
        let deleted = db.delete("nonexistent").unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_search_by_title() {
        let db = NotesDb::open_in_memory().unwrap();
        db.save(&make_note("n1", "Rust Programming", "body1", 1000, 1000))
            .unwrap();
        db.save(&make_note("n2", "Python Programming", "body2", 1000, 1000))
            .unwrap();
        db.save(&make_note("n3", "Cooking Recipes", "body3", 1000, 1000))
            .unwrap();

        let results = db.search("Programming").unwrap();
        assert_eq!(results.len(), 2);
        let ids: Vec<&str> = results.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"n1"));
        assert!(ids.contains(&"n2"));
    }

    #[test]
    fn test_search_by_content() {
        let db = NotesDb::open_in_memory().unwrap();
        db.save(&make_note("n1", "Title1", "Learn Rust today", 1000, 1000))
            .unwrap();
        db.save(&make_note("n2", "Title2", "Learn Python today", 1000, 1000))
            .unwrap();
        db.save(&make_note("n3", "Title3", "Go hiking", 1000, 1000))
            .unwrap();

        let results = db.search("Learn").unwrap();
        assert_eq!(results.len(), 2);
        let ids: Vec<&str> = results.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"n1"));
        assert!(ids.contains(&"n2"));
    }

    #[test]
    fn test_count_notes() {
        let db = NotesDb::open_in_memory().unwrap();
        assert_eq!(db.count().unwrap(), 0);

        db.save(&make_note("n1", "T1", "C1", 1000, 1000)).unwrap();
        assert_eq!(db.count().unwrap(), 1);

        db.save(&make_note("n2", "T2", "C2", 1000, 1000)).unwrap();
        assert_eq!(db.count().unwrap(), 2);

        db.delete("n1").unwrap();
        assert_eq!(db.count().unwrap(), 1);
    }

    #[test]
    fn test_list_ordered_by_updated_at_descending() {
        let db = NotesDb::open_in_memory().unwrap();
        db.save(&make_note("oldest", "Old", "Old note", 1000, 1000))
            .unwrap();
        db.save(&make_note("newest", "New", "New note", 2000, 3000))
            .unwrap();
        db.save(&make_note("middle", "Mid", "Mid note", 1500, 2000))
            .unwrap();

        let notes = db.list().unwrap();
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].id, "newest");
        assert_eq!(notes[1].id, "middle");
        assert_eq!(notes[2].id, "oldest");
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let db = NotesDb::open_in_memory().unwrap();
        let result = db.get("nonexistent").unwrap();
        assert!(result.is_none());
    }
}
