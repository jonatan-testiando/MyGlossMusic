//! Persistencia local en SQLite.
//!
//! Guarda ajustes, favoritos e historial. Todo local: no hay servidor, ni
//! sincronizacion, ni telemetria.
//!
//! El acceso va tras un `Mutex` y no un pool: las escrituras son de unos pocos
//! bytes y ocurren como mucho una vez por cancion, asi que la contencion es
//! inexistente y el codigo queda mucho mas simple.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// Una pista guardada en favoritos o en el historial.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedTrack {
    pub video_id: String,
    pub title: String,
    pub author: String,
    pub thumbnail: Option<String>,
    /// Segundos desde epoch.
    pub at: i64,
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open() -> Result<Self> {
        let path = data_dir();
        std::fs::create_dir_all(&path).ok();
        let file = path.join("posible.db");

        let conn = Connection::open(&file)
            .with_context(|| format!("no se pudo abrir {}", file.display()))?;

        // WAL: lecturas y escrituras no se bloquean entre si. Sin esto, guardar
        // el historial mientras se lee la lista de favoritos puede trabar la
        // interfaz.
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "synchronous", "NORMAL").ok();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS favorites (
                 video_id  TEXT PRIMARY KEY,
                 title     TEXT NOT NULL,
                 author    TEXT NOT NULL,
                 thumbnail TEXT,
                 added_at  INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS history (
                 id        INTEGER PRIMARY KEY AUTOINCREMENT,
                 video_id  TEXT NOT NULL,
                 title     TEXT NOT NULL,
                 author    TEXT NOT NULL,
                 thumbnail TEXT,
                 played_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_history_time ON history(played_at DESC);",
        )
        .context("no se pudo preparar el esquema")?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ---------------------------------------------------------------- ajustes

    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().ok()?;
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // -------------------------------------------------------------- favoritos

    pub fn is_favorite(&self, video_id: &str) -> bool {
        let Ok(conn) = self.conn.lock() else {
            return false;
        };
        conn.query_row(
            "SELECT 1 FROM favorites WHERE video_id = ?1",
            params![video_id],
            |_| Ok(()),
        )
        .is_ok()
    }

    /// Alterna favorito y devuelve el estado resultante.
    pub fn toggle_favorite(&self, t: &SavedTrack) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        let existed = conn.execute("DELETE FROM favorites WHERE video_id = ?1", params![t.video_id])?;
        if existed > 0 {
            return Ok(false);
        }
        conn.execute(
            "INSERT INTO favorites (video_id, title, author, thumbnail, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![t.video_id, t.title, t.author, t.thumbnail, now()],
        )?;
        Ok(true)
    }

    pub fn favorites(&self) -> Result<Vec<SavedTrack>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        let mut stmt = conn.prepare(
            "SELECT video_id, title, author, thumbnail, added_at
             FROM favorites ORDER BY added_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_track)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    // -------------------------------------------------------------- historial

    /// Registra una reproduccion, evitando duplicar la misma pista seguida.
    pub fn push_history(&self, t: &SavedTrack) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;

        let last: Option<String> = conn
            .query_row(
                // Por `id`, no por `played_at`: dos inserciones en el mismo
                // segundo empatan y el orden quedaria indeterminado.
                "SELECT video_id FROM history ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .ok();
        if last.as_deref() == Some(t.video_id.as_str()) {
            return Ok(()); // repetir una cancion no debe llenar el historial
        }

        conn.execute(
            "INSERT INTO history (video_id, title, author, thumbnail, played_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![t.video_id, t.title, t.author, t.thumbnail, now()],
        )?;
        // Poda: el historial no debe crecer sin limite.
        conn.execute(
            "DELETE FROM history WHERE id NOT IN
             (SELECT id FROM history ORDER BY id DESC LIMIT 500)",
            [],
        )?;
        Ok(())
    }

    pub fn history(&self, limit: usize) -> Result<Vec<SavedTrack>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        let mut stmt = conn.prepare(
            "SELECT video_id, title, author, thumbnail, played_at
             FROM history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], row_to_track)?;
        Ok(rows.filter_map(Result::ok).collect())
    }
}

fn row_to_track(r: &rusqlite::Row) -> rusqlite::Result<SavedTrack> {
    Ok(SavedTrack {
        video_id: r.get(0)?,
        title: r.get(1)?,
        author: r.get(2)?,
        thumbnail: r.get(3)?,
        at: r.get(4)?,
    })
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("posible-ytmusic")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Db {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE favorites (video_id TEXT PRIMARY KEY, title TEXT NOT NULL,
                 author TEXT NOT NULL, thumbnail TEXT, added_at INTEGER NOT NULL);
             CREATE TABLE history (id INTEGER PRIMARY KEY AUTOINCREMENT, video_id TEXT NOT NULL,
                 title TEXT NOT NULL, author TEXT NOT NULL, thumbnail TEXT,
                 played_at INTEGER NOT NULL);",
        )
        .unwrap();
        Db {
            conn: Mutex::new(conn),
        }
    }

    fn track(id: &str) -> SavedTrack {
        SavedTrack {
            video_id: id.into(),
            title: format!("Cancion {id}"),
            author: "Artista".into(),
            thumbnail: None,
            at: 0,
        }
    }

    #[test]
    fn los_ajustes_sobreviven() {
        let db = mem_db();
        db.set_setting("volume", "0.7").unwrap();
        assert_eq!(db.get_setting("volume").as_deref(), Some("0.7"));
        db.set_setting("volume", "0.3").unwrap();
        assert_eq!(db.get_setting("volume").as_deref(), Some("0.3"));
    }

    #[test]
    fn favorito_alterna() {
        let db = mem_db();
        let t = track("aaa");
        assert!(db.toggle_favorite(&t).unwrap(), "primero anade");
        assert!(db.is_favorite("aaa"));
        assert!(!db.toggle_favorite(&t).unwrap(), "segundo quita");
        assert!(!db.is_favorite("aaa"));
        assert!(db.favorites().unwrap().is_empty());
    }

    #[test]
    fn el_historial_no_duplica_la_misma_seguida() {
        let db = mem_db();
        db.push_history(&track("aaa")).unwrap();
        db.push_history(&track("aaa")).unwrap();
        assert_eq!(db.history(10).unwrap().len(), 1);

        db.push_history(&track("bbb")).unwrap();
        db.push_history(&track("aaa")).unwrap();
        assert_eq!(db.history(10).unwrap().len(), 3, "volver a ella si cuenta");
    }
}
