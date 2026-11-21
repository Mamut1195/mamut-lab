//! Repositorios: la única puerta entre el dominio y SQLite.
//!
//! Cada `get` **valida al leer**: reconstruye las entidades con sus constructores, de
//! modo que una fila manipulada a mano no puede convertirse en una entidad imposible
//! (por ejemplo, un ensayo que dictamina conformidad con una norma sin revisar).

use std::str::FromStr;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use mamut_core::{
    Actor, AuditEntry, AuditId, CivilDate, Length, Mass, MeasurementId, ResultId, SampleId,
    SpecimenId, TestId, Timestamp,
};
use mamut_domain::{
    Material, Measurement, ResultRef, Sample, SampleCode, SampleState, Specimen, TestRecord,
    TestState,
};
use mamut_standards::{MethodKind, NormRef, TestCode, TestDescriptor};

use crate::{Result, StorageError};

// ------------------------------------------------------------------ muestras ---

const SAMPLE_COLUMNS: &str = "id, code, material, origin, client, project, received_on, \
     responsible_id, responsible_name, state, created_at, revision";

struct RawSample {
    id: String,
    code: String,
    material: String,
    origin: String,
    client: String,
    project: String,
    received_on: String,
    responsible_id: String,
    responsible_name: String,
    state: String,
    created_at: String,
    revision: u32,
}

impl RawSample {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            code: row.get(1)?,
            material: row.get(2)?,
            origin: row.get(3)?,
            client: row.get(4)?,
            project: row.get(5)?,
            received_on: row.get(6)?,
            responsible_id: row.get(7)?,
            responsible_name: row.get(8)?,
            state: row.get(9)?,
            created_at: row.get(10)?,
            revision: row.get(11)?,
        })
    }

    fn into_sample(self) -> Result<Sample> {
        Ok(Sample {
            id: SampleId::from_str(&self.id)?,
            code: SampleCode::new(self.code)?,
            material: Material::parse(&self.material).ok_or_else(|| {
                StorageError::Corrupt(format!("material desconocido: «{}»", self.material))
            })?,
            origin: self.origin,
            client: self.client,
            project: self.project,
            received_on: CivilDate::parse(&self.received_on)?,
            responsible: Actor::new(self.responsible_id, self.responsible_name)?,
            state: parse_sample_state(&self.state)?,
            created_at: Timestamp::parse_rfc3339(&self.created_at)?,
            revision: self.revision,
        })
    }
}

pub struct SampleRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SampleRepository<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, sample: &Sample) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sample (id, code, material, origin, client, project, received_on,
                                 responsible_id, responsible_name, state, created_at, revision)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                sample.id.to_string(),
                sample.code.as_str(),
                sample.material.as_str(),
                sample.origin,
                sample.client,
                sample.project,
                sample.received_on.to_iso(),
                sample.responsible.id,
                sample.responsible.name,
                sample.state.as_str(),
                sample.created_at.to_rfc3339(),
                sample.revision,
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, id: SampleId) -> Result<Option<Sample>> {
        let raw = self
            .conn
            .query_row(
                &format!("SELECT {SAMPLE_COLUMNS} FROM sample WHERE id = ?1"),
                params![id.to_string()],
                RawSample::from_row,
            )
            .optional()?;
        raw.map(RawSample::into_sample).transpose()
    }

    pub fn find_by_code(&self, code: &str) -> Result<Option<Sample>> {
        let raw = self
            .conn
            .query_row(
                &format!("SELECT {SAMPLE_COLUMNS} FROM sample WHERE code = ?1"),
                params![code.trim().to_ascii_uppercase()],
                RawSample::from_row,
            )
            .optional()?;
        raw.map(RawSample::into_sample).transpose()
    }

    /// Muestras más recientes primero.
    pub fn list(&self, limit: usize) -> Result<Vec<Sample>> {
        let mut statement = self.conn.prepare(&format!(
            "SELECT {SAMPLE_COLUMNS} FROM sample ORDER BY created_at DESC, code ASC LIMIT ?1"
        ))?;
        let rows = statement.query_map(params![limit as i64], RawSample::from_row)?;
        let mut samples = Vec::new();
        for row in rows {
            samples.push(row?.into_sample()?);
        }
        Ok(samples)
    }

    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sample", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// Actualiza el estado y la revisión. No permite retroceder: eso lo decide el dominio.
    pub fn update_state(&self, id: SampleId, state: SampleState, revision: u32) -> Result<()> {
        self.conn.execute(
            "UPDATE sample SET state = ?1, revision = ?2 WHERE id = ?3",
            params![state.as_str(), revision, id.to_string()],
        )?;
        Ok(())
    }
}

// ---------------------------------------------------------------- especímenes ---

pub struct SpecimenRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SpecimenRepository<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, specimen: &Specimen) -> Result<()> {
        self.conn.execute(
            "INSERT INTO specimen (id, sample_id, designation, diameter_mm, height_mm, mass_g,
                                   age_days, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                specimen.id.to_string(),
                specimen.sample_id.to_string(),
                specimen.designation,
                specimen.diameter.millimetres_value(),
                specimen.height.millimetres_value(),
                specimen.mass.grams_value(),
                specimen.age_days,
                specimen.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Un espécimen por su identificador (lo necesita el historial para la edad).
    pub fn get(&self, id: SpecimenId) -> Result<Option<Specimen>> {
        let row = self
            .conn
            .query_row(
                "SELECT id, sample_id, designation, diameter_mm, height_mm, mass_g, age_days, created_at
                 FROM specimen WHERE id = ?1",
                params![id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, f64>(3)?,
                        row.get::<_, f64>(4)?,
                        row.get::<_, f64>(5)?,
                        row.get::<_, u32>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;

        let Some((id, sample, designation, diameter, height, mass, age, created)) = row else {
            return Ok(None);
        };
        Ok(Some(Specimen {
            id: SpecimenId::from_str(&id)?,
            sample_id: SampleId::from_str(&sample)?,
            designation,
            diameter: Length::millimetres(diameter)?,
            height: Length::millimetres(height)?,
            mass: Mass::grams(mass)?,
            age_days: age,
            created_at: Timestamp::parse_rfc3339(&created)?,
        }))
    }

    pub fn list_by_sample(&self, sample_id: SampleId) -> Result<Vec<Specimen>> {
        let mut statement = self.conn.prepare(
            "SELECT id, sample_id, designation, diameter_mm, height_mm, mass_g, age_days, created_at
             FROM specimen WHERE sample_id = ?1 ORDER BY designation ASC",
        )?;
        let rows = statement.query_map(params![sample_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, u32>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, sample, designation, diameter, height, mass, age, created) = row?;
            out.push(Specimen {
                id: SpecimenId::from_str(&id)?,
                sample_id: SampleId::from_str(&sample)?,
                designation,
                diameter: Length::millimetres(diameter)?,
                height: Length::millimetres(height)?,
                mass: Mass::grams(mass)?,
                age_days: age,
                created_at: Timestamp::parse_rfc3339(&created)?,
            });
        }
        Ok(out)
    }
}

// -------------------------------------------------------------------- ensayos ---

/// El descriptor viaja como objeto de valor. Se **reconstruye con su constructor** al
/// leer, así que una fila manipulada no puede saltarse las reglas.
#[derive(Serialize, Deserialize)]
struct DescriptorDto {
    code: String,
    name: String,
    method_kind: MethodKind,
    norm: Option<NormRef>,
    conformity_enabled: bool,
}

impl DescriptorDto {
    fn from_descriptor(descriptor: &TestDescriptor) -> Self {
        Self {
            code: descriptor.code.as_str().to_owned(),
            name: descriptor.name.clone(),
            method_kind: descriptor.method_kind,
            norm: descriptor.norm.clone(),
            conformity_enabled: descriptor.conformity_enabled,
        }
    }

    fn into_descriptor(self) -> Result<TestDescriptor> {
        Ok(TestDescriptor::new(
            TestCode::new(self.code)?,
            self.name,
            self.method_kind,
            self.norm,
            self.conformity_enabled,
        )?)
    }
}

const TEST_COLUMNS: &str = "id, specimen_id, test_code, descriptor_json, state, performed_on, \
     equipment, operator_id, operator_name, created_at, revision";

struct RawTest {
    id: String,
    specimen_id: String,
    descriptor_json: String,
    state: String,
    performed_on: String,
    equipment: Option<String>,
    operator_id: String,
    operator_name: String,
    created_at: String,
    revision: u32,
}

impl RawTest {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            specimen_id: row.get(1)?,
            descriptor_json: row.get(3)?,
            state: row.get(4)?,
            performed_on: row.get(5)?,
            equipment: row.get(6)?,
            operator_id: row.get(7)?,
            operator_name: row.get(8)?,
            created_at: row.get(9)?,
            revision: row.get(10)?,
        })
    }

    fn into_record(self, result: Option<ResultRef>) -> Result<TestRecord> {
        let dto: DescriptorDto = serde_json::from_str(&self.descriptor_json)
            .map_err(|e| StorageError::Corrupt(format!("descriptor ilegible: {e}")))?;
        Ok(TestRecord {
            id: TestId::from_str(&self.id)?,
            specimen_id: SpecimenId::from_str(&self.specimen_id)?,
            descriptor: dto.into_descriptor()?,
            state: parse_test_state(&self.state)?,
            performed_on: CivilDate::parse(&self.performed_on)?,
            equipment: self.equipment,
            operator: Actor::new(self.operator_id, self.operator_name)?,
            created_at: Timestamp::parse_rfc3339(&self.created_at)?,
            revision: self.revision,
            result,
        })
    }
}

pub struct TestRecordRepository<'a> {
    conn: &'a Connection,
}

impl<'a> TestRecordRepository<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, test: &TestRecord) -> Result<()> {
        let descriptor =
            serde_json::to_string(&DescriptorDto::from_descriptor(&test.descriptor))
                .map_err(|e| StorageError::Corrupt(format!("descriptor no serializable: {e}")))?;
        self.conn.execute(
            "INSERT INTO test_record (id, specimen_id, test_code, descriptor_json, state,
                                      performed_on, equipment, operator_id, operator_name,
                                      created_at, revision)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                test.id.to_string(),
                test.specimen_id.to_string(),
                test.descriptor.code.as_str(),
                descriptor,
                test.state.as_str(),
                test.performed_on.to_iso(),
                test.equipment,
                test.operator.id,
                test.operator.name,
                test.created_at.to_rfc3339(),
                test.revision,
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, id: TestId) -> Result<Option<TestRecord>> {
        let raw = self
            .conn
            .query_row(
                &format!("SELECT {TEST_COLUMNS} FROM test_record WHERE id = ?1"),
                params![id.to_string()],
                RawTest::from_row,
            )
            .optional()?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let result = ResultRepository::new(self.conn).latest_for_test(id)?;
        Ok(Some(raw.into_record(result)?))
    }

    pub fn list_by_specimen(&self, specimen_id: SpecimenId) -> Result<Vec<TestRecord>> {
        let mut statement = self.conn.prepare(&format!(
            "SELECT {TEST_COLUMNS} FROM test_record WHERE specimen_id = ?1 ORDER BY created_at ASC"
        ))?;
        let rows = statement.query_map(params![specimen_id.to_string()], RawTest::from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?.into_record(None)?);
        }
        Ok(out)
    }

    pub fn update_state(&self, id: TestId, state: TestState, revision: u32) -> Result<()> {
        self.conn.execute(
            "UPDATE test_record SET state = ?1, revision = ?2 WHERE id = ?3",
            params![state.as_str(), revision, id.to_string()],
        )?;
        Ok(())
    }

    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM test_record", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// Todos los ensayos de una muestra, pasando por sus especímenes.
    ///
    /// Es la consulta del historial: de la muestra al espécimen, al ensayo y a su
    /// resultado. Se ordena por fecha de ensayo.
    pub fn list_by_sample(&self, sample_id: SampleId) -> Result<Vec<TestRecord>> {
        let mut statement = self.conn.prepare(&format!(
            "SELECT {TEST_COLUMNS} FROM test_record
             WHERE specimen_id IN (SELECT id FROM specimen WHERE sample_id = ?1)
             ORDER BY performed_on ASC, created_at ASC"
        ))?;
        let rows = statement.query_map(params![sample_id.to_string()], RawTest::from_row)?;
        let mut out = Vec::new();
        for row in rows {
            let raw = row?;
            // El resultado se lee con la vía validada, igual que en `get`.
            let result =
                ResultRepository::new(self.conn).latest_for_test(TestId::from_str(&raw.id)?)?;
            out.push(raw.into_record(result)?);
        }
        Ok(out)
    }
}

// ----------------------------------------------------------------- resultados ---

pub struct ResultRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ResultRepository<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Inserta un resultado. La base rechaza `UPDATE` y `DELETE` sobre esta tabla.
    pub fn insert(&self, test_id: TestId, result: &ResultRef) -> Result<ResultId> {
        // Defensa en profundidad: el constructor ya valida, pero un literal construido
        // dentro del workspace no pasaría por él.
        result.validate()?;
        let id = ResultId::new();
        self.conn.execute(
            "INSERT INTO result (id, test_id, value, unit, engine_version, inputs_fingerprint,
                                 calculated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id.to_string(),
                test_id.to_string(),
                result.value,
                result.unit,
                result.engine_version,
                result.inputs_fingerprint,
                result.calculated_at.to_rfc3339(),
            ],
        )?;
        Ok(id)
    }

    pub fn list_by_test(&self, test_id: TestId) -> Result<Vec<ResultRef>> {
        let mut statement = self.conn.prepare(
            "SELECT value, unit, engine_version, inputs_fingerprint, calculated_at
             FROM result WHERE test_id = ?1 ORDER BY calculated_at DESC",
        )?;
        let rows = statement.query_map(params![test_id.to_string()], |row| {
            Ok((
                row.get::<_, f64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (value, unit, engine_version, fingerprint, calculated) = row?;
            // Se reconstruye por el constructor validado: un resultado con unidad
            // inventada o valor infinito no debe llegar nunca a un informe.
            out.push(ResultRef::new(
                value,
                unit,
                engine_version,
                fingerprint,
                Timestamp::parse_rfc3339(&calculated)?,
            )?);
        }
        Ok(out)
    }

    pub fn latest_for_test(&self, test_id: TestId) -> Result<Option<ResultRef>> {
        Ok(self.list_by_test(test_id)?.into_iter().next())
    }
}

// --------------------------------------------------------------- mediciones ---

pub struct MeasurementRepository<'a> {
    conn: &'a Connection,
}

impl<'a> MeasurementRepository<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, test_id: TestId, measurement: &Measurement) -> Result<()> {
        measurement.validate()?;
        self.conn.execute(
            "INSERT INTO measurement (id, test_id, variable, value_text, unit, revision,
                                      supersedes, correction_reason, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                measurement.id.to_string(),
                test_id.to_string(),
                measurement.variable,
                measurement.value_text,
                measurement.unit,
                measurement.revision,
                measurement.supersedes,
                measurement.correction_reason,
                measurement.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Todas las versiones de un ensayo, originales y correcciones.
    pub fn list_by_test(&self, test_id: TestId) -> Result<Vec<Measurement>> {
        let mut statement = self.conn.prepare(
            "SELECT id, variable, value_text, unit, revision, supersedes, correction_reason,
                    recorded_at
             FROM measurement WHERE test_id = ?1 ORDER BY variable ASC, revision ASC",
        )?;
        let rows = statement.query_map(params![test_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, Option<u32>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, variable, value_text, unit, revision, supersedes, reason, recorded) = row?;
            // Se reconstruye por la vía validada: una base migrada puede contener
            // unidades que la API actual ya no admite.
            out.push(Measurement::from_parts(
                MeasurementId::from_str(&id)?,
                variable,
                value_text,
                unit,
                revision,
                supersedes,
                reason,
                Timestamp::parse_rfc3339(&recorded)?,
            )?);
        }
        Ok(out)
    }
}

// ------------------------------------------------------------------ auditoría ---

pub struct AuditRepository<'a> {
    conn: &'a Connection,
}

impl<'a> AuditRepository<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Solo inserción. La base rechaza cualquier `UPDATE` o `DELETE` sobre `audit`.
    pub fn append(&self, entry: &AuditEntry) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audit (id, at, actor_id, actor_name, entity, entity_id, action, detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                entry.id.to_string(),
                entry.at.to_rfc3339(),
                entry.actor.id,
                entry.actor.name,
                entry.entity,
                entry.entity_id,
                entry.action.as_str(),
                entry.detail.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn for_entity(&self, entity: &str, entity_id: &str) -> Result<Vec<AuditEntry>> {
        let mut statement = self.conn.prepare(
            "SELECT id, at, actor_id, actor_name, entity, entity_id, action, detail
             FROM audit WHERE entity = ?1 AND entity_id = ?2 ORDER BY at ASC, id ASC",
        )?;
        let rows = statement.query_map(params![entity, entity_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, at, actor_id, actor_name, entity, entity_id, action, detail) = row?;
            out.push(AuditEntry {
                id: AuditId::from_str(&id)?,
                at: Timestamp::parse_rfc3339(&at)?,
                actor: Actor::new(actor_id, actor_name)?,
                entity,
                entity_id,
                action: mamut_core::AuditAction::parse(&action).ok_or_else(|| {
                    StorageError::Corrupt(format!("acción de auditoría desconocida: «{action}»"))
                })?,
                detail: detail
                    .and_then(|d| serde_json::from_str(&d).ok())
                    .unwrap_or(serde_json::Value::Null),
            });
        }
        Ok(out)
    }

    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }
}

// ------------------------------------------------------------------ adjuntos ---

/// Metadatos de un archivo adjunto. El contenido vive en el almacén de archivos,
/// direccionado por `sha256`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentRecord {
    pub file_name: String,
    pub mime: String,
    pub sha256: String,
    pub bytes: i64,
}

pub struct AttachmentRepository<'a> {
    conn: &'a Connection,
}

impl<'a> AttachmentRepository<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &self,
        sample_id: SampleId,
        file_name: &str,
        mime: &str,
        sha256: &str,
        bytes: i64,
        added_by: &str,
        added_at: Timestamp,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO attachment (id, sample_id, file_name, mime, sha256, bytes, added_by, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                mamut_core::AuditId::new().to_string(),
                sample_id.to_string(),
                file_name,
                mime,
                sha256,
                bytes,
                added_by,
                added_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_by_sample(&self, sample_id: SampleId) -> Result<Vec<AttachmentRecord>> {
        let mut statement = self.conn.prepare(
            "SELECT file_name, mime, sha256, bytes FROM attachment
             WHERE sample_id = ?1 ORDER BY added_at ASC, file_name ASC",
        )?;
        let rows = statement.query_map(params![sample_id.to_string()], |row| {
            Ok(AttachmentRecord {
                file_name: row.get(0)?,
                mime: row.get(1)?,
                sha256: row.get(2)?,
                bytes: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM attachment", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }
}

// -------------------------------------------------------------- ensayos de suelos ---

/// Un ensayo de suelos nuevo, con sus datos originales.
pub struct NewSoilTest<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub descriptor_code: &'a str,
    /// Datos originales en JSON, tal como se capturaron.
    pub inputs: &'a str,
    pub result_value: f64,
    pub result_unit: &'a str,
    pub engine_version: &'a str,
    pub fingerprint: &'a str,
    pub performed_on: &'a str,
    pub created_by: &'a str,
    pub created_at: &'a str,
}

/// Un ensayo de suelos ya guardado.
#[derive(Clone, Debug, PartialEq)]
pub struct SoilTestRecord {
    pub id: String,
    pub kind: String,
    pub descriptor_code: String,
    pub result_value: f64,
    pub result_unit: String,
    pub performed_on: String,
}

pub struct SoilTestRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SoilTestRepository<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, sample_id: SampleId, test: &NewSoilTest<'_>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO soil_test (
                 id, sample_id, kind, descriptor_code, inputs, result_value, result_unit,
                 engine_version, fingerprint, performed_on, created_by, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                test.id,
                sample_id.to_string(),
                test.kind,
                test.descriptor_code,
                test.inputs,
                test.result_value,
                test.result_unit,
                test.engine_version,
                test.fingerprint,
                test.performed_on,
                test.created_by,
                test.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_by_sample(&self, sample_id: SampleId) -> Result<Vec<SoilTestRecord>> {
        let mut statement = self.conn.prepare(
            "SELECT id, kind, descriptor_code, result_value, result_unit, performed_on
             FROM soil_test WHERE sample_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = statement.query_map(params![sample_id.to_string()], |row| {
            Ok(SoilTestRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                descriptor_code: row.get(2)?,
                result_value: row.get(3)?,
                result_unit: row.get(4)?,
                performed_on: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM soil_test", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }
}

// --------------------------------------------------------------------- informes ---

/// Un informe emitido.
pub struct NewReport<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub subject: &'a str,
    pub path: &'a str,
    pub sha256: &'a str,
    pub issued_by: &'a str,
    pub issued_at: &'a str,
}

/// Un informe ya registrado.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportRecord {
    pub id: String,
    pub kind: String,
    pub subject: String,
    pub path: String,
    pub sha256: String,
    pub issued_at: String,
}

pub struct ReportRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ReportRepository<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, sample_id: SampleId, report: &NewReport<'_>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO report (id, kind, subject, sample_id, path, sha256, issued_by, issued_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                report.id,
                report.kind,
                report.subject,
                sample_id.to_string(),
                report.path,
                report.sha256,
                report.issued_by,
                report.issued_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_by_sample(&self, sample_id: SampleId) -> Result<Vec<ReportRecord>> {
        let mut statement = self.conn.prepare(
            "SELECT id, kind, subject, path, sha256, issued_at
             FROM report WHERE sample_id = ?1 ORDER BY issued_at ASC",
        )?;
        let rows = statement.query_map(params![sample_id.to_string()], |row| {
            Ok(ReportRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                subject: row.get(2)?,
                path: row.get(3)?,
                sha256: row.get(4)?,
                issued_at: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM report", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }
}

// -------------------------------------------------------------------- ayudas ---

fn parse_sample_state(text: &str) -> Result<SampleState> {
    Ok(match text {
        "received" => SampleState::Received,
        "in_testing" => SampleState::InTesting,
        "completed" => SampleState::Completed,
        "voided" => SampleState::Voided,
        other => {
            return Err(StorageError::Corrupt(format!(
                "estado de muestra desconocido: «{other}»"
            )));
        }
    })
}

fn parse_test_state(text: &str) -> Result<TestState> {
    Ok(match text {
        "draft" => TestState::Draft,
        "registered" => TestState::Registered,
        "calculated" => TestState::Calculated,
        "reported" => TestState::Reported,
        "voided" => TestState::Voided,
        other => {
            return Err(StorageError::Corrupt(format!(
                "estado de ensayo desconocido: «{other}»"
            )));
        }
    })
}
