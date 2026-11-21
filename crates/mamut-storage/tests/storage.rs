#![allow(clippy::unwrap_used, clippy::expect_used)]
// Las pruebas usan unwrap/expect a propósito: si una precondición falla,
// queremos que la prueba estalle justo ahí.

//! Pruebas de persistencia por la **superficie pública**.
//!
//! Los ataques que necesitan SQL crudo (intentar reescribir un resultado, desactivar un
//! disparador, manipular una fila) viven en `src/adversarial.rs`, dentro del crate: el
//! acceso a la conexión no forma parte del API precisamente para que la aplicación no
//! pueda saltarse las garantías.

use mamut_core::{Actor, CivilDate, Length, Mass, SampleId, Timestamp};
use mamut_domain::{
    Material, Measurement, ResultRef, Sample, SampleCode, SampleState, Specimen, TestRecord,
    TestState,
};
use mamut_standards::{MethodKind, NormBody, NormRef, NormStatus, TestCode, TestDescriptor};
use mamut_storage::Database;

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

fn sample_with_code(code: &str) -> Sample {
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
    let sample = sample_with_code("H-2026-0001");
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

// --------------------------------------------------------------- muestras ---

#[test]
fn una_muestra_va_y_vuelve_intacta() {
    let db = db();
    let sample = sample_with_code("H-2026-0001");
    db.samples().insert(&sample).unwrap();

    let recovered = db.samples().get(sample.id).unwrap().unwrap();
    assert_eq!(recovered.code.as_str(), "H-2026-0001");
    assert_eq!(recovered.material, Material::Concrete);
    assert_eq!(recovered.origin, "Bloque 3");
    assert_eq!(recovered.client, "Constructora Andina");
    assert_eq!(recovered.project, "Torre Norte");
    assert_eq!(recovered.received_on.to_iso(), "2026-11-18");
    assert_eq!(recovered.responsible.name, "Ana Torres");
    assert_eq!(recovered.state, SampleState::Received);
    assert_eq!(recovered.revision, 1);
    assert_eq!(recovered.id, sample.id);
}

#[test]
fn el_codigo_de_muestra_es_unico_en_la_base() {
    let db = db();
    db.samples()
        .insert(&sample_with_code("H-2026-0001"))
        .unwrap();
    assert!(
        db.samples()
            .insert(&sample_with_code("H-2026-0001"))
            .is_err(),
        "dos muestras no pueden compartir código"
    );
}

#[test]
fn buscar_por_codigo_ignora_mayusculas_y_espacios() {
    let db = db();
    let sample = sample_with_code("H-2026-0001");
    db.samples().insert(&sample).unwrap();
    let found = db.samples().find_by_code("  h-2026-0001  ").unwrap();
    assert_eq!(found.map(|s| s.id), Some(sample.id));
}

#[test]
fn el_listado_devuelve_la_mas_reciente_primero() {
    let db = db();

    let older = Sample::new(
        SampleCode::new("H-1").unwrap(),
        Material::Soil,
        "Calicata 1",
        "Cliente",
        "Proyecto",
        CivilDate::new(2026, 11, 1).unwrap(),
        actor(),
        Timestamp::from_millis(1_000),
    )
    .unwrap();
    let newer = Sample::new(
        SampleCode::new("H-2").unwrap(),
        Material::Concrete,
        "Bloque 3",
        "Cliente",
        "Proyecto",
        CivilDate::new(2026, 11, 2).unwrap(),
        actor(),
        Timestamp::from_millis(2_000),
    )
    .unwrap();

    db.samples().insert(&older).unwrap();
    db.samples().insert(&newer).unwrap();

    let list = db.samples().list(10).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].code.as_str(), "H-2", "la más reciente va primero");
    assert_eq!(db.samples().count().unwrap(), 2);
}

#[test]
fn actualizar_el_estado_persiste_y_sube_la_revision() {
    let db = db();
    let mut sample = sample_with_code("H-1");
    db.samples().insert(&sample).unwrap();
    sample.transition(SampleState::InTesting).unwrap();
    db.samples()
        .update_state(sample.id, sample.state, sample.revision)
        .unwrap();

    let recovered = db.samples().get(sample.id).unwrap().unwrap();
    assert_eq!(recovered.state, SampleState::InTesting);
    assert_eq!(recovered.revision, 2);
}

// ------------------------------------------------------------- integridad ---

#[test]
fn un_espécimen_no_puede_apuntar_a_una_muestra_inexistente() {
    let db = db();
    let orphan = Specimen::new(
        SampleId::new(),
        "C1",
        Length::millimetres(150.0).unwrap(),
        Length::millimetres(300.0).unwrap(),
        Mass::grams(12_300.0).unwrap(),
        28,
        now(),
    )
    .unwrap();
    assert!(
        db.specimens().insert(&orphan).is_err(),
        "las claves foráneas deben impedir huérfanos"
    );
}

// ---------------------------------------------------------------- cadena ---

#[test]
fn la_cadena_muestra_espécimen_ensayo_se_guarda_y_se_lee() {
    let db = db();
    let (sample, specimen, test) = chain(&db);

    let specimens = db.specimens().list_by_sample(sample.id).unwrap();
    assert_eq!(specimens.len(), 1);
    assert_eq!(specimens[0].designation, "C1");
    assert!((specimens[0].diameter.millimetres_value() - 150.0).abs() < 1e-9);

    let recovered = db.tests().get(test.id).unwrap().unwrap();
    assert_eq!(recovered.specimen_id, specimen.id);
    assert_eq!(recovered.descriptor.code.as_str(), "CONC-COMP-CYL");
    assert_eq!(recovered.state, TestState::Draft);
    assert!(recovered.result.is_none());
    assert_eq!(
        recovered.descriptor.norm.as_ref().map(NormRef::citation),
        Some("ASTM C39/C39M-26".to_owned())
    );
}

#[test]
fn el_estado_del_ensayo_se_actualiza_y_el_resultado_se_lee() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);

    db.results().insert(test.id, &result_ref()).unwrap();
    db.tests()
        .update_state(test.id, TestState::Calculated, 2)
        .unwrap();

    let recovered = db.tests().get(test.id).unwrap().unwrap();
    assert_eq!(recovered.state, TestState::Calculated);
    assert_eq!(recovered.revision, 2);
    let result = recovered
        .result
        .expect("el resultado debe volver con el ensayo");
    assert!((result.value - 28.29).abs() < 1e-12);
    assert_eq!(result.unit, "MPa");
}

#[test]
fn dos_resultados_del_mismo_ensayo_se_conservan_en_orden() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);

    let first = ResultRef::new(28.29, "MPa", "0.1.0", "a".repeat(64), now()).unwrap();
    let later = ResultRef::new(
        28.31,
        "MPa",
        "0.1.1",
        "c".repeat(64),
        Timestamp::from_millis(now().millis() + 1_000),
    )
    .unwrap();
    db.results().insert(test.id, &first).unwrap();
    db.results().insert(test.id, &later).unwrap();

    let all = db.results().list_by_test(test.id).unwrap();
    assert_eq!(
        all.len(),
        2,
        "cada cálculo es una fila nueva, no una sustitución"
    );
    assert_eq!(all[0].engine_version, "0.1.1", "el más reciente primero");
    assert_eq!(
        db.results()
            .latest_for_test(test.id)
            .unwrap()
            .unwrap()
            .value,
        28.31
    );
}

// -------------------------------------------------------------- mediciones ---

#[test]
fn las_mediciones_conservan_el_original_y_la_correccion() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);

    let original = Measurement::new("masa", "1230.0", "g", now()).unwrap();
    db.measurements().insert(test.id, &original).unwrap();
    let correction = original
        .corrected("1230.5", "lectura mal transcrita", now())
        .unwrap();
    db.measurements().insert(test.id, &correction).unwrap();

    let stored = db.measurements().list_by_test(test.id).unwrap();
    assert_eq!(
        stored.len(),
        2,
        "una corrección no sustituye: añade una versión"
    );
    assert_eq!(stored[0].value_text, "1230.0");
    assert_eq!(stored[1].value_text, "1230.5");
    assert_eq!(stored[1].supersedes, Some(1));
    assert_eq!(
        stored[1].correction_reason.as_deref(),
        Some("lectura mal transcrita")
    );
}

#[test]
fn una_correccion_duplicada_de_la_misma_revision_es_rechazada() {
    let db = db();
    let (_sample, _specimen, test) = chain(&db);

    let original = Measurement::new("masa", "1230.0", "g", now()).unwrap();
    db.measurements().insert(test.id, &original).unwrap();
    let correction = original.corrected("1230.5", "motivo", now()).unwrap();
    db.measurements().insert(test.id, &correction).unwrap();

    // La misma variable no puede tener dos versiones 2.
    let duplicate = original.corrected("1231.0", "otro motivo", now()).unwrap();
    assert!(
        db.measurements().insert(test.id, &duplicate).is_err(),
        "la unicidad (test, variable, revision) debe impedirlo"
    );
}

// ------------------------------------------------------------------ copias ---

#[test]
fn una_copia_de_seguridad_se_crea_y_se_verifica() {
    let db = db();
    let _ = chain(&db);

    let dir = std::env::temp_dir().join(format!("mamut-backup-{}", std::process::id()));
    let path = dir.join("copia.db");
    let _ = std::fs::remove_dir_all(&dir);

    db.backup_to(&path).unwrap();
    assert!(path.exists(), "la copia debe existir");

    // La copia se puede abrir y contiene los datos.
    let restored = Database::open(&path).unwrap();
    assert_eq!(restored.samples().count().unwrap(), 1);
    assert_eq!(restored.schema_version().unwrap(), 4);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_se_sobrescribe_una_copia_existente() {
    let db = db();
    let dir = std::env::temp_dir().join(format!("mamut-backup2-{}", std::process::id()));
    let path = dir.join("copia.db");
    let _ = std::fs::remove_dir_all(&dir);

    db.backup_to(&path).unwrap();
    let second = db.backup_to(&path);
    assert!(
        second.is_err(),
        "sobrescribir una copia es una pérdida silenciosa"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn la_base_abierta_pasa_el_control_de_integridad() {
    let db = db();
    db.verify_integrity().unwrap();
}
