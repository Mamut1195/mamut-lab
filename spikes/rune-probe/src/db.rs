//! Probe: SQLite persistence in-process.
//!
//! Only proves that the storage dependency works on Windows without a system
//! SQLite (bundled build) and that a schema + query round trip behaves. The real
//! schema, migrations and repositories belong to `lab-storage` in Fase 1.

use rusqlite::{Connection, OptionalExtension, params};

pub fn open_in_memory() -> rusqlite::Result<Connection> {
    Connection::open_in_memory()
}

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS sample (
             id          TEXT PRIMARY KEY,
             material    TEXT NOT NULL,
             received_at TEXT NOT NULL
         );",
    )
}

pub fn insert_sample(conn: &Connection, id: &str, material: &str, received_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sample (id, material, received_at) VALUES (?1, ?2, ?3)",
        params![id, material, received_at],
    )?;
    Ok(())
}

pub fn get_sample(conn: &Connection, id: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT material FROM sample WHERE id = ?1", params![id], |row| row.get::<_, String>(0))
        .optional()
}
