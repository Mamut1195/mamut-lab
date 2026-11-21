//! Puerta de escritura de la aplicación.
//!
//! La vista no toca SQLite: pide al `Store`. Así la interfaz sigue siendo una función
//! del estado y el acceso a datos está en un solo sitio, fácil de sustituir cuando el
//! almacenamiento pase a su propio hilo (ADR-0003).
//!
//! **Desviación conocida:** en la fase 2 las escrituras ocurren en el hilo de la interfaz.
//! Un `INSERT` local tarda microsegundos y el usuario está esperando el resultado, así que
//! el coste es despreciable; moverlo a un hilo con canal es deuda (DT-19).

use std::path::Path;

use mamut_core::{Actor, AuditAction, AuditEntry, Timestamp};
use mamut_domain::Sample;
use mamut_storage::Database;

use crate::state::SampleRow;

/// Error de almacenamiento ya traducido a algo que se puede mostrar al usuario.
pub type StoreResult<T> = std::result::Result<T, String>;

/// Acceso a la base local.
pub struct Store {
    db: Database,
}

impl Store {
    /// Abre (o crea) la base y aplica las migraciones pendientes.
    pub fn open(path: &Path, now: Timestamp) -> StoreResult<Self> {
        let mut db = Database::open(path).map_err(display)?;
        db.migrate(now).map_err(display)?;
        Ok(Self { db })
    }

    /// Base en memoria, para pruebas.
    pub fn in_memory(now: Timestamp) -> StoreResult<Self> {
        let mut db = Database::open_in_memory().map_err(display)?;
        db.migrate(now).map_err(display)?;
        Ok(Self { db })
    }

    /// Guarda una muestra y deja constancia en la auditoría.
    ///
    /// Las dos escrituras van en la misma llamada: si la auditoría fallara, la muestra ya
    /// estaría guardada, así que el error se reporta y la interfaz puede reintentar el
    /// registro de auditoría. La base no permite borrar la muestra para «deshacer».
    pub fn create_sample(&self, sample: &Sample, actor: &Actor, at: Timestamp) -> StoreResult<()> {
        self.db.samples().insert(sample).map_err(display)?;
        let entry = AuditEntry::new(
            at,
            actor.clone(),
            "sample",
            sample.id.to_string(),
            AuditAction::Created,
        )
        .map_err(display)?;
        self.db.audit().append(&entry).map_err(display)?;
        Ok(())
    }

    /// Muestras más recientes, ya formateadas para la tabla.
    pub fn sample_rows(&self, limit: usize) -> StoreResult<Vec<SampleRow>> {
        let samples = self.db.samples().list(limit).map_err(display)?;
        Ok(samples.iter().map(SampleRow::from_sample).collect())
    }

    /// Número total de muestras (no solo las cargadas en la tabla).
    pub fn sample_count(&self) -> StoreResult<usize> {
        self.db.samples().count().map_err(display)
    }

    /// Número total de ensayos registrados.
    pub fn test_count(&self) -> StoreResult<usize> {
        self.db.tests().count().map_err(display)
    }

    /// Entradas de auditoría de una entidad (para pruebas y para el detalle futuro).
    pub fn audit_entries(&self, entity: &str, id: &str) -> StoreResult<usize> {
        self.db
            .audit()
            .for_entity(entity, id)
            .map(|entries| entries.len())
            .map_err(display)
    }
}

fn display<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mamut_core::{CivilDate, Timestamp};
    use mamut_domain::{Material, SampleCode};

    fn now() -> Timestamp {
        Timestamp::from_millis(1_763_500_000_000)
    }

    fn actor() -> Actor {
        Actor::new("local", "Ana Torres").unwrap()
    }

    fn sample(code: &str) -> Sample {
        Sample::new(
            SampleCode::new(code).unwrap(),
            Material::Concrete,
            "Bloque 3",
            "Constructora Andina",
            "Torre Norte",
            CivilDate::new(2026, 11, 18).unwrap(),
            actor(),
            now(),
        )
        .unwrap()
    }

    #[test]
    fn guardar_una_muestra_la_deja_consultable_y_auditada() {
        let store = Store::in_memory(now()).unwrap();
        let muestra = sample("H-2026-0001");
        store.create_sample(&muestra, &actor(), now()).unwrap();

        let rows = store.sample_rows(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].code, "H-2026-0001");
        assert_eq!(store.sample_count().unwrap(), 1);
        assert_eq!(
            store
                .audit_entries("sample", &muestra.id.to_string())
                .unwrap(),
            1,
            "el alta debe quedar registrada en la auditoría"
        );
    }

    #[test]
    fn dos_muestras_con_el_mismo_codigo_no_entran() {
        let store = Store::in_memory(now()).unwrap();
        store
            .create_sample(&sample("H-1"), &actor(), now())
            .unwrap();
        assert!(
            store
                .create_sample(&sample("H-1"), &actor(), now())
                .is_err(),
            "la segunda debe fallar y el error debe ser legible"
        );
        assert_eq!(store.sample_count().unwrap(), 1);
    }

    #[test]
    fn la_tabla_respeta_el_limite_pedido() {
        let store = Store::in_memory(now()).unwrap();
        for index in 0..5 {
            store
                .create_sample(&sample(&format!("H-{index}")), &actor(), now())
                .unwrap();
        }
        assert_eq!(store.sample_rows(2).unwrap().len(), 2);
        assert_eq!(store.sample_count().unwrap(), 5, "el total no se recorta");
    }

    #[test]
    fn sin_ensayos_el_contador_es_cero() {
        let store = Store::in_memory(now()).unwrap();
        assert_eq!(store.test_count().unwrap(), 0);
    }
}
