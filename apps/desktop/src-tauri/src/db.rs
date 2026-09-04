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

/// Una playlist local del usuario.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub count: i64,
    /// Portada de la primera pista, para la tarjeta de la lista.
    pub thumbnail: Option<String>,
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
             CREATE INDEX IF NOT EXISTS idx_history_time ON history(played_at DESC);

             CREATE TABLE IF NOT EXISTS playlists (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 name       TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             -- `position` y no el orden de insercion: una playlist se reordena,
             -- y sin una columna propia no habria como guardar ese orden.
             CREATE TABLE IF NOT EXISTS playlist_items (
                 playlist_id INTEGER NOT NULL
                     REFERENCES playlists(id) ON DELETE CASCADE,
                 video_id    TEXT    NOT NULL,
                 title       TEXT    NOT NULL,
                 author      TEXT    NOT NULL,
                 thumbnail   TEXT,
                 position    INTEGER NOT NULL,
                 added_at    INTEGER NOT NULL,
                 PRIMARY KEY (playlist_id, video_id)
             );
             CREATE INDEX IF NOT EXISTS idx_items_orden
                 ON playlist_items(playlist_id, position);",
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

    // ------------------------------------------------------- metadatos rotos

    /// Pistas guardadas de las que no se llego a saber el nombre.
    ///
    /// Antes de que existiera el repuesto de metadatos, una pista cuyo stream
    /// no se pudo resolver se guardaba con el titulo de relleno, y ahi se
    /// quedaba. Esto las encuentra para poder arreglarlas.
    pub fn tracks_sin_titulo(&self, limit: usize) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        let mut stmt = conn.prepare(
            "SELECT video_id, MAX(cuando) FROM (
                 SELECT video_id, title, played_at AS cuando FROM history
                 UNION ALL
                 SELECT video_id, title, added_at   AS cuando FROM favorites
                 UNION ALL
                 SELECT video_id, title, added_at   AS cuando FROM playlist_items
             )
             WHERE title = ?1 OR title = ''
             GROUP BY video_id
             ORDER BY 2 DESC
             LIMIT ?2",
        )?;
        let filas = stmt
            .query_map(params![ytm_audio::SIN_TITULO, limit as i64], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(filas)
    }

    /// Rellena el nombre de una pista alla donde se haya guardado sin el.
    ///
    /// Las tres tablas a la vez: la misma cancion puede estar en el historial,
    /// en favoritos y en varias playlists, y arreglarla en una sola dejaria las
    /// otras con el "Sin titulo".
    pub fn fill_track_meta(
        &self,
        video_id: &str,
        title: &str,
        author: &str,
        thumbnail: Option<&str>,
    ) -> Result<usize> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        let mut tocadas = 0;
        for tabla in ["history", "favorites", "playlist_items"] {
            tocadas += conn.execute(
                &format!(
                    "UPDATE {tabla} SET title = ?1, author = ?2,
                            thumbnail = COALESCE(?3, thumbnail)
                     WHERE video_id = ?4 AND (title = ?5 OR title = '')"
                ),
                params![title, author, thumbnail, video_id, ytm_audio::SIN_TITULO],
            )?;
        }
        Ok(tocadas)
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

    // -------------------------------------------------------------- playlists

    pub fn create_playlist(&self, name: &str) -> Result<i64> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        conn.execute(
            "INSERT INTO playlists (name, created_at) VALUES (?1, ?2)",
            params![name.trim(), now()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        conn.execute("UPDATE playlists SET name = ?2 WHERE id = ?1", params![id, name.trim()])?;
        Ok(())
    }

    pub fn delete_playlist(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        // `ON DELETE CASCADE` solo actua si las claves ajenas estan activas, y
        // en SQLite vienen apagadas por conexion. Se borran las pistas a mano
        // para no depender de ese pragma.
        conn.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", params![id])?;
        conn.execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name,
                    (SELECT COUNT(*) FROM playlist_items i WHERE i.playlist_id = p.id),
                    (SELECT i.thumbnail FROM playlist_items i
                      WHERE i.playlist_id = p.id ORDER BY i.position LIMIT 1)
               FROM playlists p ORDER BY p.created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Playlist { id: r.get(0)?, name: r.get(1)?, count: r.get(2)?, thumbnail: r.get(3)? })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn playlist_tracks(&self, id: i64) -> Result<Vec<SavedTrack>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        let mut stmt = conn.prepare(
            "SELECT video_id, title, author, thumbnail, added_at
               FROM playlist_items WHERE playlist_id = ?1 ORDER BY position",
        )?;
        let rows = stmt.query_map(params![id], row_to_track)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Anade al final. Si la pista ya estaba, no la duplica ni la mueve.
    pub fn add_to_playlist(&self, id: i64, t: &SavedTrack) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        let siguiente: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_items WHERE playlist_id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT OR IGNORE INTO playlist_items
                 (playlist_id, video_id, title, author, thumbnail, position, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, t.video_id, t.title, t.author, t.thumbnail, siguiente, now()],
        )?;
        Ok(())
    }

    pub fn remove_from_playlist(&self, id: i64, video_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("mutex envenenado"))?;
        conn.execute(
            "DELETE FROM playlist_items WHERE playlist_id = ?1 AND video_id = ?2",
            params![id, video_id],
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
                 played_at INTEGER NOT NULL);
             CREATE TABLE playlists (id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL, created_at INTEGER NOT NULL);
             CREATE TABLE playlist_items (playlist_id INTEGER NOT NULL, video_id TEXT NOT NULL,
                 title TEXT NOT NULL, author TEXT NOT NULL, thumbnail TEXT,
                 position INTEGER NOT NULL, added_at INTEGER NOT NULL,
                 PRIMARY KEY (playlist_id, video_id));",
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

    #[test]
    fn una_playlist_conserva_el_orden_en_que_se_anadio() {
        let db = mem_db();
        let id = db.create_playlist("De noche").unwrap();
        for x in ["a", "b", "c"] {
            db.add_to_playlist(id, &track(x)).unwrap();
        }
        let ids: Vec<String> = db
            .playlist_tracks(id)
            .unwrap()
            .into_iter()
            .map(|t| t.video_id)
            .collect();
        assert_eq!(ids, ["a", "b", "c"], "el orden no es el de insercion");
    }

    #[test]
    fn anadir_dos_veces_la_misma_pista_no_la_duplica() {
        let db = mem_db();
        let id = db.create_playlist("Repes").unwrap();
        db.add_to_playlist(id, &track("a")).unwrap();
        db.add_to_playlist(id, &track("a")).unwrap();
        assert_eq!(db.playlist_tracks(id).unwrap().len(), 1);
    }

    #[test]
    fn borrar_una_playlist_se_lleva_sus_pistas() {
        // `ON DELETE CASCADE` no basta: en SQLite las claves ajenas vienen
        // apagadas por conexion, asi que las pistas se borran a mano.
        let db = mem_db();
        let id = db.create_playlist("Efimera").unwrap();
        db.add_to_playlist(id, &track("a")).unwrap();
        db.delete_playlist(id).unwrap();

        assert!(db.playlists().unwrap().is_empty());
        assert!(db.playlist_tracks(id).unwrap().is_empty(), "quedaron pistas huerfanas");
    }

    #[test]
    fn la_lista_de_playlists_trae_cuenta_y_portada() {
        let db = mem_db();
        let id = db.create_playlist("Con portada").unwrap();
        // El ayudante `track` no trae portada; aqui hace falta para comprobar
        // que la tarjeta coge la de la PRIMERA pista y no otra.
        let mut primera = track("a");
        primera.thumbnail = Some("https://ejemplo/a.jpg".into());
        db.add_to_playlist(id, &primera).unwrap();
        let mut segunda = track("b");
        segunda.thumbnail = Some("https://ejemplo/b.jpg".into());
        db.add_to_playlist(id, &segunda).unwrap();

        let listas = db.playlists().unwrap();
        assert_eq!(listas.len(), 1);
        assert_eq!(listas[0].count, 2);
        assert_eq!(
            listas[0].thumbnail.as_deref(),
            Some("https://ejemplo/a.jpg"),
            "deberia coger la portada de la primera pista"
        );
    }

    #[test]
    fn quitar_una_pista_no_toca_las_demas() {
        let db = mem_db();
        let id = db.create_playlist("Menos una").unwrap();
        for x in ["a", "b", "c"] {
            db.add_to_playlist(id, &track(x)).unwrap();
        }
        db.remove_from_playlist(id, "b").unwrap();
        let ids: Vec<String> = db
            .playlist_tracks(id)
            .unwrap()
            .into_iter()
            .map(|t| t.video_id)
            .collect();
        assert_eq!(ids, ["a", "c"]);
    }
}
