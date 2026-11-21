//! Copias de seguridad verificadas.
//!
//! Una copia que no se ha comprobado no es una copia: es una carpeta con un archivo
//! dentro. Aquí se crea con `VACUUM INTO` (una foto consistente, compactada) y se
//! verifica abriéndola y ejecutando `PRAGMA integrity_check`.

use std::path::Path;

use rusqlite::Connection;

use crate::{Database, Result, StorageError};

impl Database {
    /// Crea una copia en `path` y **verifica** que la copia es íntegra.
    ///
    /// Falla si el destino ya existe: sobrescribir una copia anterior sin querer es una
    /// forma silenciosa de perder datos.
    pub fn backup_to(&self, path: &Path) -> Result<()> {
        if path.exists() {
            return Err(StorageError::BackupExists(path.display().to_string()));
        }
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                StorageError::Corrupt(format!("no se pudo crear el directorio de copias: {error}"))
            })?;
        }

        // `VACUUM INTO` no admite parámetros; se escapa la comilla simple.
        let target = path.to_string_lossy().replace('\'', "''");
        self.connection()
            .execute_batch(&format!("VACUUM INTO '{target}'"))?;

        let copy = Connection::open(path)?;
        let integrity: String = copy.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(StorageError::BackupCorrupt);
        }
        Ok(())
    }

    /// Comprueba la integridad de la base abierta.
    pub fn verify_integrity(&self) -> Result<()> {
        let integrity: String =
            self.connection()
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(StorageError::BackupCorrupt);
        }
        Ok(())
    }
}
