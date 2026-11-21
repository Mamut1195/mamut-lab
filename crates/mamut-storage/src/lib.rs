//! `mamut-storage` — SQLite, migraciones y repositorios.
//!
//! **Lo que esta capa garantiza por construcción, no por disciplina:**
//! - El registro de **auditoría** es append-only: la base rechaza `UPDATE` y `DELETE`.
//! - Un **resultado calculado** no se modifica ni se borra: la base lo rechaza.
//!
//! Si esa garantía viviera solo en el código, un `UPDATE` despistado la rompería en
//! silencio. Aquí la rompe el motor y la prueba lo comprueba (ADR-0006 §4).

pub mod backup;
pub mod repos;

#[cfg(test)]
mod adversarial;

use std::path::Path;

use mamut_core::Timestamp;
use rusqlite::{Connection, params};

pub use repos::{
    AttachmentRecord, AttachmentRepository, AuditRepository, MeasurementRepository, NewReport,
    NewSoilTest, ReportRecord, ReportRepository, ResultRepository, SampleRepository,
    SoilTestRecord, SoilTestRepository, SpecimenRepository, TestRecordRepository,
};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("error de SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("error de dominio: {0}")]
    Domain(#[from] mamut_domain::DomainError),

    #[error("error de normas: {0}")]
    Standards(#[from] mamut_standards::StandardsError),

    #[error("error de núcleo: {0}")]
    Core(#[from] mamut_core::Error),

    #[error("dato corrupto en la base: {0}")]
    Corrupt(String),

    #[error("la migración {version} falló: {message}")]
    Migration { version: i64, message: String },

    #[error("ya existe un archivo en {0}")]
    BackupExists(String),

    #[error("la copia de seguridad no superó la verificación de integridad")]
    BackupCorrupt,

    #[error(
        "la base de datos usa el esquema {found}, más nuevo que el que entiende esta \
         aplicación ({supported}): actualice el programa antes de abrirla"
    )]
    SchemaTooNew { found: i64, supported: i64 },
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Migraciones en orden. Cada entrada se aplica una sola vez y en su propia transacción.
pub const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../migrations/V001__init.sql")),
    (
        2,
        include_str!("../migrations/V002__measurement_immutable.sql"),
    ),
    (
        3,
        include_str!("../migrations/V003__unit_and_value_guards.sql"),
    ),
    (
        4,
        include_str!("../migrations/V004__normalize_legacy_units.sql"),
    ),
    (5, include_str!("../migrations/V005__attachments.sql")),
    (
        6,
        include_str!("../migrations/V006__soil_tests_and_reports.sql"),
    ),
];

/// Versión de esquema que conoce esta compilación.
pub const SCHEMA_VERSION: i64 = 6;

/// La base de datos local.
pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::configure(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure(conn)
    }

    fn configure(conn: Connection) -> Result<Self> {
        // `foreign_keys` se activa por conexión: sin esto, las claves foráneas del
        // esquema serían decorativas.
        //
        // `recursive_triggers` importa más de lo que parece: sin él, un
        // `INSERT OR REPLACE` borra la fila en conflicto **sin disparar** los
        // disparadores de borrado, y la inmutabilidad de `audit` y `result` se podría
        // saltar en una línea. Con él, el `REPLACE` dispara el `BEFORE DELETE` y aborta.
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA recursive_triggers = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;",
        )?;
        Ok(Self { conn })
    }

    /// Acceso a la conexión, **solo dentro del crate**.
    ///
    /// No es público a propósito: con un `&Connection` en la mano se pueden desactivar
    /// los PRAGMA o borrar los disparadores, y entonces la inmutabilidad deja de ser una
    /// garantía del producto. Las pruebas que necesitan SQL directo viven dentro del
    /// crate (`src/tests.rs`) precisamente para que la superficie pública no la exponga.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Aplica las migraciones pendientes. Devuelve las versiones aplicadas.
    ///
    /// Es idempotente: llamarla dos veces no aplica nada la segunda vez.
    pub fn migrate(&mut self, at: Timestamp) -> Result<Vec<i64>> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                 version    INTEGER PRIMARY KEY,
                 applied_at TEXT NOT NULL
             );",
        )?;

        let current: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?;

        // Una base creada por una versión posterior del programa no se puede abrir a
        // ciegas: el esquema tendría columnas que este código no conoce. Es el caso
        // típico tras un rollback de versión.
        if current > SCHEMA_VERSION {
            return Err(StorageError::SchemaTooNew {
                found: current,
                supported: SCHEMA_VERSION,
            });
        }

        let mut applied = Vec::new();
        for (version, sql) in MIGRATIONS {
            if *version <= current {
                continue;
            }
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)
                .map_err(|error| StorageError::Migration {
                    version: *version,
                    message: error.to_string(),
                })?;
            tx.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                params![version, at.to_rfc3339()],
            )?;
            tx.commit()?;
            applied.push(*version);
        }
        Ok(applied)
    }

    /// Ejecuta SQL suelto. Solo para pruebas de los disparadores de inmutabilidad.
    pub fn execute_raw(&self, sql: &str) -> Result<()> {
        self.conn.execute_batch(sql)?;
        Ok(())
    }

    /// Versión de esquema aplicada actualmente (0 si nunca se migró).
    pub fn schema_version(&self) -> Result<i64> {
        let exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
            [],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Ok(0);
        }
        Ok(self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn samples(&self) -> SampleRepository<'_> {
        SampleRepository::new(&self.conn)
    }

    pub fn specimens(&self) -> SpecimenRepository<'_> {
        SpecimenRepository::new(&self.conn)
    }

    pub fn tests(&self) -> TestRecordRepository<'_> {
        TestRecordRepository::new(&self.conn)
    }

    pub fn results(&self) -> ResultRepository<'_> {
        ResultRepository::new(&self.conn)
    }

    pub fn measurements(&self) -> MeasurementRepository<'_> {
        MeasurementRepository::new(&self.conn)
    }

    pub fn audit(&self) -> AuditRepository<'_> {
        AuditRepository::new(&self.conn)
    }

    pub fn attachments(&self) -> AttachmentRepository<'_> {
        AttachmentRepository::new(&self.conn)
    }

    pub fn soil_tests(&self) -> SoilTestRepository<'_> {
        SoilTestRepository::new(&self.conn)
    }

    pub fn reports(&self) -> ReportRepository<'_> {
        ReportRepository::new(&self.conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> Timestamp {
        Timestamp::from_millis(1_763_500_000_000)
    }

    #[test]
    fn una_base_nueva_esta_en_version_cero() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), 0);
    }

    #[test]
    fn migrar_aplica_todas_las_versiones() {
        let mut db = Database::open_in_memory().unwrap();
        assert_eq!(db.migrate(now()).unwrap(), vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn migrar_dos_veces_no_aplica_nada_la_segunda() {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate(now()).unwrap();
        assert_eq!(db.migrate(now()).unwrap(), Vec::<i64>::new());
    }

    #[test]
    fn las_claves_foraneas_estan_activas() {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate(now()).unwrap();
        let on: i64 = db
            .conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(on, 1, "sin foreign_keys las referencias no se comprueban");
    }

    #[test]
    fn el_esquema_crea_todas_las_tablas_esperadas() {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate(now()).unwrap();
        for table in [
            "sample",
            "specimen",
            "test_record",
            "measurement",
            "result",
            "equipment",
            "audit",
            "schema_version",
        ] {
            let count: i64 = db
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "falta la tabla {table}");
        }
    }
}
