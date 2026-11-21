#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Pruebas adversariales de la capa de almacenamiento.
//!
//! Viven **dentro** del crate porque necesitan SQL crudo, y el SQL crudo no forma parte
//! de la superficie pública: `Database::connection()` es `pub(crate)` precisamente para
//! que el código de la aplicación no pueda desactivar los PRAGMA ni borrar disparadores.
//!
//! Lo que se prueba aquí no es que SQLite funcione, sino que **las garantías del
//! producto resisten un ataque deliberado**, no solo un uso descuidado.

use std::str::FromStr;

use mamut_core::{
    Actor, AuditAction, AuditEntry, CivilDate, Length, Mass, SampleId, SpecimenId, Timestamp,
};
use mamut_domain::{
    Material, Measurement as DomainMeasurement, ResultRef, Sample, SampleCode, Specimen, TestRecord,
};
use mamut_standards::{MethodKind, NormBody, NormRef, NormStatus, TestCode, TestDescriptor};
use rusqlite::params;

use crate::Database;

fn now() -> Timestamp {
    Timestamp::from_millis(1_763_500_000_000)
}

fn actor() -> Actor {
    Actor::new("u-1", "Ana Torres").unwrap()
}

fn db() -> Database {
    let mut db = Database::open_in_memory().unwrap();
    db.migrate(now()).unwrap();
    db
}

fn sample() -> Sample {
    Sample::new(
        SampleCode::new("H-2026-0001").unwrap(),
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

fn descriptor() -> TestDescriptor {
    let norm = NormRef::new(
        NormBody::Astm,
        "C39/C39M",
        2026,
        "Compresión de cilindros",
        NormStatus::PendingReview,
    )
    .unwrap();
    TestDescriptor::new(
        TestCode::new("CONC-COMP-CYL").unwrap(),
        "Compresión de cilindros",
        MethodKind::Normative,
        Some(norm),
        false,
    )
    .unwrap()
}

/// Cadena completa: muestra → espécimen → ensayo.
fn chain(db: &Database) -> (Sample, Specimen, TestRecord) {
    let sample = sample();
    db.samples().insert(&sample).unwrap();

    let specimen = Specimen::new(
        sample.id,
        "C1",
        Length::millimetres(150.0).unwrap(),
        Length::millimetres(300.0).unwrap(),
        Mass::grams(12_300.0).unwrap(),
        28,
        now(),
    )
    .unwrap();
    db.specimens().insert(&specimen).unwrap();

    let test = TestRecord::new(
        descriptor(),
        specimen.id,
        CivilDate::new(2026, 11, 18).unwrap(),
        actor(),
        now(),
    );
    db.tests().insert(&test).unwrap();
    (sample, specimen, test)
}

fn result_ref() -> ResultRef {
    ResultRef::new(28.29, "MPa", "0.1.0", "b".repeat(64), now()).unwrap()
}

fn entry() -> AuditEntry {
    AuditEntry::new(now(), actor(), "sample", "s-1", AuditAction::Created)
        .unwrap()
        .with_detail(serde_json::json!({"nota": "prueba"}))
}

// ------------------------------------------------------------- configuración ---

#[test]
fn los_pragmas_de_proteccion_estan_activos() {
    let db = db();
    let foreign_keys: i64 = db
        .connection()
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    let recursive: i64 = db
        .connection()
        .query_row("PRAGMA recursive_triggers", [], |row| row.get(0))
        .unwrap();
    assert_eq!(foreign_keys, 1);
    assert_eq!(
        recursive, 1,
        "sin recursive_triggers, INSERT OR REPLACE esquiva los disparadores de borrado"
    );
}

#[test]
fn existen_los_seis_disparadores_de_inmutabilidad() {
    let db = db();
    let mut names: Vec<String> = db
        .connection()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'trigger' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    names.sort();
    for expected in [
        "audit_no_delete",
        "audit_no_update",
        "measurement_no_delete",
        "measurement_no_update",
        "result_no_delete",
        "result_no_update",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "falta el disparador {expected}; hay: {names:?}"
        );
    }
}

// ------------------------------------------------------------ filas corruptas ---

#[test]
fn una_fila_manipulada_a_mano_no_se_convierte_en_entidad_valida() {
    let db = db();
    let sample = sample();
    db.samples().insert(&sample).unwrap();

    db.connection()
        .execute(
            "UPDATE sample SET state = 'inventado' WHERE id = ?1",
            [sample.id.to_string()],
        )
        .unwrap();

    let error = db.samples().get(sample.id).unwrap_err();
    assert!(
        error.to_string().contains("estado de muestra desconocido"),
        "se esperaba un error de corrupción, llegó: {error}"
    );
}

#[test]
fn un_descriptor_manipulado_no_puede_habilitar_conformidad() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);

    let tampered = r#"{
        "code": "CONC-COMP-CYL",
        "name": "Compresión de cilindros",
        "method_kind": "Normative",
        "norm": {
            "body": "ASTM",
            "designation": "C39/C39M",
            "year": 2026,
            "title": "Compresión de cilindros",
            "status": "PendingReview"
        },
        "conformity_enabled": true
    }"#;
    db.connection()
        .execute(
            "UPDATE test_record SET descriptor_json = ?1 WHERE id = ?2",
            params![tampered, test.id.to_string()],
        )
        .unwrap();

    let error = db.tests().get(test.id).unwrap_err();
    assert!(
        error.to_string().contains("norma revisada"),
        "se esperaba el rechazo por conformidad sin norma revisada, llegó: {error}"
    );
}

// ------------------------------------------------------- resultados inmutables ---

#[test]
fn un_resultado_calculado_no_se_puede_modificar() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);
    db.results().insert(test.id, &result_ref()).unwrap();

    let error = db
        .connection()
        .execute("UPDATE result SET value = 99.0", [])
        .unwrap_err();
    assert!(
        error.to_string().contains("no se modifica"),
        "el disparador debe impedir la modificación: {error}"
    );
}

#[test]
fn un_resultado_calculado_no_se_puede_borrar() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);
    db.results().insert(test.id, &result_ref()).unwrap();

    let error = db
        .connection()
        .execute("DELETE FROM result", [])
        .unwrap_err();
    assert!(
        error.to_string().contains("no se borra"),
        "el disparador debe impedir el borrado: {error}"
    );
}

#[test]
fn un_insert_or_replace_no_puede_sobrescribir_un_resultado() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);
    db.results().insert(test.id, &result_ref()).unwrap();

    let id: String = db
        .connection()
        .query_row("SELECT id FROM result LIMIT 1", [], |row| row.get(0))
        .unwrap();

    let attack = db.connection().execute(
        "INSERT OR REPLACE INTO result (id, test_id, value, unit, engine_version,
                                        inputs_fingerprint, calculated_at)
         VALUES (?1, ?2, 99.0, 'MPa', 'falso', 'mala', '2026-11-18T00:00:00.000Z')",
        params![id, test.id.to_string()],
    );

    assert!(
        attack.is_err(),
        "un resultado no se puede reemplazar ni con INSERT OR REPLACE"
    );
    assert!(
        (db.results()
            .latest_for_test(test.id)
            .unwrap()
            .unwrap()
            .value
            - 28.29)
            .abs()
            < 1e-12,
        "el resultado original debe seguir intacto"
    );
}

// ---------------------------------------------------------- auditoría inmutable ---

#[test]
fn la_auditoria_no_se_puede_modificar() {
    let db = db();
    db.audit().append(&entry()).unwrap();

    let error = db
        .connection()
        .execute("UPDATE audit SET action = 'voided'", [])
        .unwrap_err();
    assert!(
        error.to_string().contains("append-only"),
        "el disparador debe impedir la modificación: {error}"
    );
}

#[test]
fn la_auditoria_no_se_puede_borrar() {
    let db = db();
    db.audit().append(&entry()).unwrap();

    let error = db
        .connection()
        .execute("DELETE FROM audit", [])
        .unwrap_err();
    assert!(
        error.to_string().contains("append-only"),
        "el disparador debe impedir el borrado: {error}"
    );
}

#[test]
fn un_insert_or_replace_no_puede_reescribir_la_auditoria() {
    let db = db();
    db.audit().append(&entry()).unwrap();

    let id: String = db
        .connection()
        .query_row("SELECT id FROM audit LIMIT 1", [], |row| row.get(0))
        .unwrap();

    let attack = db.connection().execute(
        "INSERT OR REPLACE INTO audit (id, at, actor_id, actor_name, entity, entity_id, action, detail)
         VALUES (?1, '2026-11-18T00:00:00.000Z', 'x', 'Falsificador', 'sample', 's-1', 'voided', NULL)",
        params![id],
    );

    assert!(attack.is_err(), "la auditoría no se puede reescribir");
    let entries = db.audit().for_entity("sample", "s-1").unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].action, AuditAction::Created);
}

// -------------------------------------------------------- mediciones inmutables ---

#[test]
fn una_medicion_no_se_puede_modificar_ni_borrar() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);
    let original = DomainMeasurement::new("masa", "1230.0", "g", now()).unwrap();
    db.measurements().insert(test.id, &original).unwrap();

    let update = db
        .connection()
        .execute("UPDATE measurement SET value_text = '9999.0'", [])
        .unwrap_err();
    assert!(
        update.to_string().contains("no se modifica"),
        "el dato original no se puede sobrescribir: {update}"
    );

    let delete = db
        .connection()
        .execute("DELETE FROM measurement", [])
        .unwrap_err();
    assert!(
        delete.to_string().contains("no se borra"),
        "el dato original no se puede borrar: {delete}"
    );

    let stored = db.measurements().list_by_test(test.id).unwrap();
    assert_eq!(stored[0].value_text, "1230.0");
}

#[test]
fn una_correccion_sin_motivo_es_rechazada_por_la_base() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);

    let error = db
        .connection()
        .execute(
            "INSERT INTO measurement (id, test_id, variable, value_text, unit, revision, supersedes,
                                      correction_reason, recorded_at)
             VALUES ('m-x', ?1, 'masa', '1240.0', 'g', 2, 1, NULL, '2026-11-18T00:00:00.000Z')",
            params![test.id.to_string()],
        )
        .unwrap_err();
    assert!(
        error.to_string().to_lowercase().contains("check"),
        "la restricción CHECK debe impedir una corrección sin motivo: {error}"
    );
}

#[test]
fn la_base_rechaza_una_unidad_inventada_y_un_valor_infinito() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);

    let bad_unit = db.connection().execute(
        "INSERT INTO measurement (id, test_id, variable, value_text, unit, revision,
                                  supersedes, correction_reason, recorded_at)
         VALUES ('m-1', ?1, 'masa', '1', 'mts', 1, NULL, NULL, '2026-11-18T00:00:00.000Z')",
        params![test.id.to_string()],
    );
    assert!(
        bad_unit.is_err(),
        "la base debe rechazar una unidad que no está en la lista"
    );

    let bad_value = db.connection().execute(
        "INSERT INTO result (id, test_id, value, unit, engine_version, inputs_fingerprint,
                             calculated_at)
         VALUES ('r-1', ?1, 1e400, 'MPa', 'v1', 'aaa', '2026-11-18T00:00:00.000Z')",
        params![test.id.to_string()],
    );
    assert!(
        bad_value.is_err(),
        "la base debe rechazar un resultado infinito (quedaría grabado para siempre)"
    );
}

#[test]
fn migrar_desde_la_v1_conserva_datos_y_añade_protecciones() {
    // Simula una instalación antigua: solo la migración 1 aplicada y datos dentro.
    let mut db = Database::open_in_memory().unwrap();
    db.connection()
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                 version INTEGER PRIMARY KEY,
                 applied_at TEXT NOT NULL
             );",
        )
        .unwrap();
    db.connection()
        .execute_batch(crate::MIGRATIONS[0].1)
        .unwrap();
    db.connection()
        .execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (1, '2026-11-18T00:00:00.000Z')",
            [],
        )
        .unwrap();

    let sample = sample();
    db.samples().insert(&sample).unwrap();

    let specimen = Specimen::new(
        sample.id,
        "C1",
        Length::millimetres(150.0).unwrap(),
        Length::millimetres(300.0).unwrap(),
        Mass::grams(12_300.0).unwrap(),
        28,
        now(),
    )
    .unwrap();
    db.specimens().insert(&specimen).unwrap();

    let test = TestRecord::new(
        descriptor(),
        specimen.id,
        CivilDate::new(2026, 11, 18).unwrap(),
        actor(),
        now(),
    );
    db.tests().insert(&test).unwrap();

    // Una base v1 permitía unidades escritas a mano: se simula una fila así. El
    // identificador debe ser un UUID válido porque la lectura lo valida.
    db.connection()
        .execute(
            "INSERT INTO measurement (id, test_id, variable, value_text, unit, revision,
                                      supersedes, correction_reason, recorded_at)
             VALUES ('0192f0a0-1234-7abc-8def-0123456789ab', ?1, 'masa', '1230.0',
                     'milímetros', 1, NULL, NULL, '2026-11-18T00:00:00.000Z')",
            params![test.id.to_string()],
        )
        .unwrap();
    assert_eq!(db.schema_version().unwrap(), 1);

    let applied = db.migrate(now()).unwrap();
    assert_eq!(
        applied,
        vec![2, 3, 4, 5, 6],
        "se aplican las migraciones pendientes"
    );
    assert_eq!(db.schema_version().unwrap(), 6);
    assert_eq!(
        db.samples().count().unwrap(),
        1,
        "migrar no puede perder datos"
    );
    assert_eq!(
        db.samples().get(sample.id).unwrap().unwrap().code.as_str(),
        "H-2026-0001"
    );

    // La fila heredada con «milímetros» queda legible: la v4 la normaliza a «mm».
    let measurements = db.measurements().list_by_test(test.id).unwrap();
    assert_eq!(measurements.len(), 1);
    assert_eq!(
        measurements[0].unit, "mm",
        "una unidad heredada debe poder leerse tras migrar, no quedar atrapada"
    );
    assert_eq!(measurements[0].value_text, "1230.0");

    // Y la inmutabilidad nueva ya está activa.
    let blocked = db
        .connection()
        .execute("UPDATE measurement SET value_text = '2'", []);
    assert!(
        blocked.is_err(),
        "tras migrar, la inmutabilidad de las mediciones ya está activa"
    );
}

// -------------------------------------------------------------- versiones ---

#[test]
fn una_base_de_una_version_futura_se_rechaza() {
    let mut db = db();
    db.connection()
        .execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (99, '2030-01-01T00:00:00.000Z')",
            [],
        )
        .unwrap();

    let error = db.migrate(now()).unwrap_err();
    assert!(
        error.to_string().contains("más nuevo"),
        "abrir una base más nueva debe fallar con un mensaje claro: {error}"
    );
}

#[test]
fn los_identificadores_guardados_son_uuids_validos() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);
    assert!(
        mamut_core::TestId::from_str(&test.id.to_string()).is_ok(),
        "el identificador debe poder leerse de vuelta"
    );
    assert!(SpecimenId::from_str(&test.specimen_id.to_string()).is_ok());
    assert!(SampleId::from_str(&_sample.id.to_string()).is_ok());
}
