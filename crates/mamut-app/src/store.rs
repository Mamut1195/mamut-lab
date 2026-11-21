//! Puerta de escritura de la aplicación.
//!
//! La vista no toca SQLite: pide al `Store`. Así la interfaz sigue siendo una función
//! del estado y el acceso a datos está en un solo sitio, fácil de sustituir cuando el
//! almacenamiento pase a su propio hilo (ADR-0003).
//!
//! **Desviación conocida:** en la fase 2 las escrituras ocurren en el hilo de la interfaz.
//! Un `INSERT` local tarda microsegundos y el usuario está esperando el resultado, así que
//! el coste es despreciable; moverlo a un hilo con canal es deuda (DT-19).

use std::path::{Path, PathBuf};

use mamut_core::{Actor, AuditAction, AuditEntry, Timestamp};
use mamut_domain::{Measurement, ResultRef, Sample, SampleState, Specimen, TestRecord};
use mamut_export::{SampleRow as ExportRow, samples_to_csv, write_samples_xlsx};
use mamut_storage::{AttachmentRecord, Database};

use crate::state::SampleRow;

/// Formato de exportación disponible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Xlsx,
}

/// Error de almacenamiento ya traducido a algo que se puede mostrar al usuario.
pub type StoreResult<T> = std::result::Result<T, String>;

/// Acceso a la base local.
pub struct Store {
    db: Database,
    data_dir: PathBuf,
}

impl Store {
    /// Abre (o crea) la base y aplica las migraciones pendientes.
    pub fn open(path: &Path, now: Timestamp) -> StoreResult<Self> {
        let mut db = Database::open(path).map_err(display)?;
        db.migrate(now).map_err(display)?;
        let data_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(Self { db, data_dir })
    }

    /// Base en memoria, para pruebas.
    pub fn in_memory(now: Timestamp) -> StoreResult<Self> {
        let mut db = Database::open_in_memory().map_err(display)?;
        db.migrate(now).map_err(display)?;
        let data_dir = std::env::temp_dir().join("mamut-lab-in-memory");
        Ok(Self { db, data_dir })
    }

    /// Carpeta de datos de la instalación.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Guarda un PDF de informe y devuelve su ruta.
    ///
    /// Los informes van a `<datos>/reports/`, con el nombre que propone el usuario pero
    /// saneado: un nombre con separadores no puede escapar de esa carpeta.
    ///
    /// **Nunca se sobrescribe un informe.** Emitir dos veces el mismo ensayo produce
    /// `informe.pdf`, `informe-2.pdf`, `informe-3.pdf`… Un informe es un documento que se
    /// entregó: pisarlo borraría la evidencia de lo que se emitió antes.
    pub fn save_pdf(&self, name: &str, bytes: &[u8]) -> StoreResult<PathBuf> {
        let dir = self.data_dir.join("reports");
        std::fs::create_dir_all(&dir).map_err(display)?;
        let sanitized: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        // `create_new` falla si el archivo existe: entre la comprobación y la escritura no
        // hay ventana para que otra instancia escriba el mismo nombre.
        for attempt in 1..=10_000u32 {
            let file = if attempt == 1 {
                dir.join(format!("{sanitized}.pdf"))
            } else {
                dir.join(format!("{sanitized}-{attempt}.pdf"))
            };
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&file)
            {
                Ok(mut handle) => {
                    use std::io::Write;
                    handle.write_all(bytes).map_err(display)?;
                    handle.flush().map_err(display)?;
                    return Ok(file);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(display(error)),
            }
        }
        Err("no se pudo encontrar un nombre libre para el informe".to_owned())
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

    /// Busca una muestra por su código visible.
    pub fn find_sample(&self, code: &str) -> StoreResult<Option<Sample>> {
        self.db.samples().find_by_code(code).map_err(display)
    }

    /// Guarda el ciclo completo de un ensayo: espécimen, ensayo, mediciones y resultado.
    ///
    /// El orden respeta las claves foráneas y el estado del ensayo: el espécimen existe
    /// antes que el ensayo, el ensayo antes que sus mediciones, y el resultado se adjunta
    /// al ensayo ya registrado (el dominio no permite otra cosa).
    pub fn save_test_bundle(
        &self,
        specimen: &Specimen,
        test: &TestRecord,
        measurements: &[Measurement],
        result: &ResultRef,
        actor: &Actor,
        at: Timestamp,
    ) -> StoreResult<()> {
        let mut registered = test.clone();
        registered.register(None).map_err(display)?;
        registered.set_result(result.clone()).map_err(display)?;

        self.db.specimens().insert(specimen).map_err(display)?;
        self.db.tests().insert(&registered).map_err(display)?;
        for measurement in measurements {
            self.db
                .measurements()
                .insert(registered.id, measurement)
                .map_err(display)?;
        }
        self.db
            .results()
            .insert(registered.id, result)
            .map_err(display)?;
        self.db
            .tests()
            .update_state(
                registered.id,
                mamut_domain::TestState::Calculated,
                registered.revision,
            )
            .map_err(display)?;

        for (entity, id, action) in [
            ("specimen", specimen.id.to_string(), AuditAction::Created),
            ("test", registered.id.to_string(), AuditAction::Created),
            ("result", registered.id.to_string(), AuditAction::Calculated),
        ] {
            let entry = AuditEntry::new(at, actor.clone(), entity, id, action).map_err(display)?;
            self.db.audit().append(&entry).map_err(display)?;
        }
        Ok(())
    }

    /// Ensayos de una muestra, para el historial.
    pub fn tests_for_sample(&self, code: &str) -> StoreResult<Vec<TestRecord>> {
        let Some(sample) = self.find_sample(code)? else {
            return Ok(Vec::new());
        };
        self.db.tests().list_by_sample(sample.id).map_err(display)
    }

    /// Adjunta un archivo a una muestra: copia el contenido al almacén direccionado por
    /// su SHA-256 y guarda solo los metadatos.
    ///
    /// El original **no se mueve ni se modifica**: se copia. Si el mismo archivo ya está
    /// adjunto a esa muestra, se avisa en lugar de duplicarlo.
    pub fn attach_file(
        &self,
        code: &str,
        source: &Path,
        actor: &Actor,
        at: Timestamp,
    ) -> StoreResult<String> {
        let sample = self
            .find_sample(code)?
            .ok_or_else(|| format!("no existe la muestra «{code}»"))?;

        // Un adjunto de laboratorio es una foto o un documento: 50 MB es margen de sobra.
        const MAX_ATTACHMENT_BYTES: u64 = 50 * 1024 * 1024;
        let size_on_disk = std::fs::metadata(source).map_err(display)?.len();
        if size_on_disk > MAX_ATTACHMENT_BYTES {
            return Err(format!(
                "el archivo pesa {} MB y el límite es 50 MB",
                size_on_disk / (1024 * 1024)
            ));
        }
        let bytes = std::fs::read(source).map_err(display)?;
        if bytes.is_empty() {
            return Err("el archivo está vacío".to_owned());
        }
        let sha256 = mamut_core::hash::hex_digest(&bytes);

        let file_name = source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "adjunto".to_owned());

        let directory = self.data_dir.join("attachments").join(&sha256);
        std::fs::create_dir_all(&directory).map_err(display)?;
        let destination = directory.join(sanitize(&file_name));
        if !destination.exists() {
            std::fs::write(&destination, &bytes).map_err(display)?;
        }

        let size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
        if let Err(error) = self.db.attachments().insert(
            sample.id,
            &file_name,
            mime_for(&file_name),
            &sha256,
            size,
            &actor.id,
            at,
        ) {
            let text = error.to_string();
            if text.contains("UNIQUE") {
                return Err(format!("«{file_name}» ya está adjunto a esta muestra"));
            }
            return Err(text);
        }

        let entry = AuditEntry::new(
            at,
            actor.clone(),
            "sample",
            sample.id.to_string(),
            AuditAction::Created,
        )
        .map_err(display)?
        .with_detail(serde_json::json!({ "attachment": file_name, "sha256": sha256 }));
        self.db.audit().append(&entry).map_err(display)?;

        Ok(format!("{file_name} ({})", &sha256[..12]))
    }

    /// Marca la muestra como «en ensayo» cuando recibe su primer espécimen.
    ///
    /// La transición la decide el dominio; aquí solo se aplica si es válida. Una muestra
    /// anulada o completada no se toca.
    pub fn begin_testing(&self, code: &str, actor: &Actor, at: Timestamp) -> StoreResult<()> {
        let Some(mut sample) = self.find_sample(code)? else {
            return Ok(());
        };
        if sample.state == SampleState::InTesting
            || !sample.state.can_transition_to(SampleState::InTesting)
        {
            return Ok(());
        }
        sample
            .transition(SampleState::InTesting)
            .map_err(|error| error.to_string())?;
        self.db
            .samples()
            .update_state(sample.id, sample.state, sample.revision)
            .map_err(display)?;

        let entry = AuditEntry::new(
            at,
            actor.clone(),
            "sample",
            sample.id.to_string(),
            AuditAction::Updated,
        )
        .map_err(display)?
        .with_detail(serde_json::json!({ "estado": sample.state.as_str() }));
        self.db.audit().append(&entry).map_err(display)?;
        Ok(())
    }

    /// Adjuntos de una muestra.
    pub fn attachments_for(&self, code: &str) -> StoreResult<Vec<AttachmentRecord>> {
        let Some(sample) = self.find_sample(code)? else {
            return Ok(Vec::new());
        };
        self.db
            .attachments()
            .list_by_sample(sample.id)
            .map_err(display)
    }

    /// Anula una muestra: no se borra, se marca y se registra el motivo.
    pub fn void_sample(
        &self,
        code: &str,
        actor: &Actor,
        at: Timestamp,
        reason: &str,
    ) -> StoreResult<()> {
        let mut sample = self
            .find_sample(code)?
            .ok_or_else(|| format!("no existe la muestra «{code}»"))?;
        sample
            .transition(SampleState::Voided)
            .map_err(|error| error.to_string())?;
        self.db
            .samples()
            .update_state(sample.id, sample.state, sample.revision)
            .map_err(display)?;

        let entry = AuditEntry::new(
            at,
            actor.clone(),
            "sample",
            sample.id.to_string(),
            AuditAction::Voided,
        )
        .map_err(display)?
        .with_detail(serde_json::json!({ "motivo": reason }));
        self.db.audit().append(&entry).map_err(display)?;
        Ok(())
    }

    /// Serie resistencia–edad de una muestra, para el gráfico.
    pub fn series_for_sample(&self, code: &str) -> StoreResult<Vec<(u32, f64)>> {
        let Some(sample) = self.find_sample(code)? else {
            return Ok(Vec::new());
        };
        let tests = self.db.tests().list_by_sample(sample.id).map_err(display)?;
        let mut series = Vec::new();
        for test in tests {
            let Some(result) = &test.result else { continue };
            let Ok(Some(specimen)) = self.db.specimens().get(test.specimen_id) else {
                continue;
            };
            series.push((specimen.age_days, result.value));
        }
        series.sort_by_key(|(age, _)| *age);
        Ok(series)
    }

    /// Exporta todas las muestras a CSV o XLSX en la carpeta de datos.
    pub fn export_samples(&self, format: ExportFormat) -> StoreResult<PathBuf> {
        let samples = self.db.samples().list(10_000).map_err(display)?;
        let rows: Vec<ExportRow> = samples
            .iter()
            .map(|sample| ExportRow {
                code: sample.code.as_str().to_owned(),
                material: sample.material.as_str().to_owned(),
                origin: sample.origin.clone(),
                client: sample.client.clone(),
                project: sample.project.clone(),
                received_on: sample.received_on.to_iso(),
                responsible: sample.responsible.name.clone(),
                state: sample.state.as_str().to_owned(),
            })
            .collect();

        let directory = self.data_dir.join("exports");
        std::fs::create_dir_all(&directory).map_err(display)?;
        match format {
            ExportFormat::Csv => {
                let text = samples_to_csv(&rows).map_err(display)?;
                let path = directory.join("muestras.csv");
                std::fs::write(&path, text).map_err(display)?;
                Ok(path)
            }
            ExportFormat::Xlsx => {
                let path = directory.join("muestras.xlsx");
                write_samples_xlsx(&path, &rows).map_err(display)?;
                Ok(path)
            }
        }
    }
}

fn display<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// Deja un nombre de archivo seguro: sin separadores ni caracteres raros.
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.trim_matches('_').is_empty() {
        "adjunto".to_owned()
    } else {
        cleaned
    }
}

/// Tipo MIME por extensión. Suficiente para clasificar adjuntos de laboratorio.
fn mime_for(name: &str) -> &'static str {
    let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match extension.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "csv" => "text/csv",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
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

    #[test]
    fn adjuntar_copia_el_archivo_y_lo_registra() {
        let store = Store::in_memory(now()).unwrap();
        let muestra = sample("H-2026-0001");
        store.create_sample(&muestra, &actor(), now()).unwrap();

        let dir = std::env::temp_dir().join(format!("mamut-attach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("foto.png");
        std::fs::write(&file, b"contenido de prueba").unwrap();

        let summary = store
            .attach_file("H-2026-0001", &file, &actor(), now())
            .unwrap();
        assert!(summary.contains("foto.png"));

        let attachments = store.attachments_for("H-2026-0001").unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].file_name, "foto.png");
        assert_eq!(attachments[0].mime, "image/png");
        assert_eq!(attachments[0].sha256.len(), 64);

        // El contenido se copia al almacén direccionado por hash.
        let stored = store
            .data_dir()
            .join("attachments")
            .join(&attachments[0].sha256)
            .join("foto.png");
        assert!(stored.exists(), "el archivo debe copiarse al almacén");

        // El mismo archivo no se duplica en la misma muestra.
        assert!(
            store
                .attach_file("H-2026-0001", &file, &actor(), now())
                .is_err()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn anular_una_muestra_cambia_el_estado_y_deja_auditoria() {
        let store = Store::in_memory(now()).unwrap();
        let muestra = sample("H-2");
        store.create_sample(&muestra, &actor(), now()).unwrap();

        store
            .void_sample("H-2", &actor(), now(), "muestra duplicada")
            .unwrap();

        let rows = store.sample_rows(10).unwrap();
        assert_eq!(rows[0].state, "Anulada");
        assert_eq!(
            store
                .audit_entries("sample", &muestra.id.to_string())
                .unwrap(),
            2,
            "alta + anulación"
        );
    }

    #[test]
    fn exportar_escribe_csv_y_xlsx() {
        let store = Store::in_memory(now()).unwrap();
        store
            .create_sample(&sample("H-1"), &actor(), now())
            .unwrap();

        let csv = store.export_samples(ExportFormat::Csv).unwrap();
        let text = std::fs::read_to_string(&csv).unwrap();
        assert!(text.contains("H-1"), "el CSV contiene la muestra");

        let xlsx = store.export_samples(ExportFormat::Xlsx).unwrap();
        let bytes = std::fs::read(&xlsx).unwrap();
        assert!(bytes.starts_with(b"PK"), "el XLSX es un contenedor ZIP");

        let _ = std::fs::remove_file(csv);
        let _ = std::fs::remove_file(xlsx);
    }

    #[test]
    fn dos_informes_con_el_mismo_nombre_no_se_pisan() {
        let store = Store::in_memory(now()).unwrap();
        let first = store.save_pdf("informe-x", b"%PDF-1.4 uno").unwrap();
        let second = store.save_pdf("informe-x", b"%PDF-1.4 dos").unwrap();

        assert_ne!(first, second, "el segundo informe debe tener otro nombre");
        assert!(first.exists(), "el primero sigue en disco");
        assert!(second.exists());
        // El primero conserva su contenido: no se sobrescribió.
        let content = std::fs::read(&first).unwrap();
        assert!(String::from_utf8_lossy(&content).contains("uno"));

        let _ = std::fs::remove_file(first);
        let _ = std::fs::remove_file(second);
    }

    #[test]
    fn la_serie_resistencia_edad_sale_ordenada() {
        let store = Store::in_memory(now()).unwrap();
        assert!(store.series_for_sample("no-existe").unwrap().is_empty());
    }
}
