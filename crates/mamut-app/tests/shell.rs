#![allow(clippy::unwrap_used, clippy::expect_used)]
// Las pruebas usan unwrap/expect a propósito: si una precondición falla,
// queremos que la prueba estalle justo ahí.

//! Pruebas del armazón de la aplicación, **sin ventana**.
//!
//! Se ejecutan con el reloj virtual de Rune: el mismo código que dibuja en pantalla se
//! monta en memoria, se localizan elementos por su clave y se simula entrada real.

use mamut_app::{AppState, Nav, SampleRow, Store, view};
use mamut_core::Timestamp;
use mamut_standards::{MethodKind, NormBody, NormRef, NormStatus, TestCode, TestDescriptor};
use rune::{Env, Key, KeyEvent, Mods, Runtime};

fn rows(count: usize) -> Vec<SampleRow> {
    (0..count)
        .map(|index| SampleRow {
            code: format!("H-2026-{index:04}"),
            material: "Hormigón".to_owned(),
            client: "Constructora Andina".to_owned(),
            received_on: "2026-11-18".to_owned(),
            state: "Recibida".to_owned(),
        })
        .collect()
}

fn runtime(sample_count: usize) -> Runtime<AppState> {
    let env = Env {
        system_dark: true,
        scale: 2.0,
    };
    let mut rt = Runtime::new(
        AppState::new(rows(sample_count)),
        view,
        (1200.0, 800.0),
        env,
    );
    // El mismo manejador que instala `main.rs` en la ventana.
    rt.on_key = Some(mamut_app::window_key);
    rt.frame(0.0);
    rt
}

fn chord(key: char) -> KeyEvent {
    KeyEvent {
        key: Key::Command(key),
        mods: Mods {
            shift: false,
            cmd: true,
            alt: false,
            ctrl: false,
            logo: false,
        },
    }
}

fn escape() -> KeyEvent {
    KeyEvent {
        key: Key::Escape,
        mods: Mods {
            shift: false,
            cmd: false,
            alt: false,
            ctrl: false,
            logo: false,
        },
    }
}

/// Avanza el reloj virtual hasta que las transiciones de presencia terminan.
///
/// Un elemento que sale sigue dibujándose mientras dura su animación de salida: si se
/// comprueba `presented` al frame siguiente, todavía está. No es un fallo, es cómo
/// funciona la presencia en Rune.
fn settle(rt: &mut Runtime<AppState>) {
    for _ in 0..120 {
        rt.frame(1.0 / 60.0);
    }
}

#[test]
fn el_armazon_se_monta_con_todas_sus_zonas() {
    let rt = runtime(3);
    for key in ["sidebar", "header", "kpis", "table-frame", "status"] {
        assert!(
            rt.presented(key).is_some(),
            "la zona «{key}» debería estar dispuesta y ser visible"
        );
    }
}

#[test]
fn la_navegacion_responde_al_clic() {
    let mut rt = runtime(0);
    assert_eq!(rt.state.nav, Nav::Dashboard);

    let (x, y) = rt.find("nav-4").expect("el elemento de navegación existe");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);

    assert_eq!(rt.state.nav, Nav::Equipment);
}

#[test]
fn el_atajo_de_teclado_abre_y_cierra_la_paleta() {
    let mut rt = runtime(0);
    assert!(
        rt.presented("palette").is_none(),
        "la paleta empieza cerrada"
    );

    rt.key(chord('k'));
    rt.frame(1.0 / 60.0);
    assert!(rt.state.palette_open, "Ctrl/⌘+K abre la paleta");
    assert!(rt.presented("palette").is_some(), "y se dibuja");

    rt.key(escape());
    rt.frame(1.0 / 60.0);
    assert!(!rt.state.palette_open, "Esc la cierra");
    settle(&mut rt);
    assert!(
        rt.presented("palette").is_none(),
        "una vez asentada la salida, la paleta ya no se dibuja"
    );
}

#[test]
fn los_atajos_numericos_cambian_de_seccion() {
    let mut rt = runtime(0);

    rt.key(chord('3'));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.nav, Nav::Concrete);

    rt.key(chord('6'));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.nav, Nav::Reports);
}

#[test]
fn el_boton_de_tema_fuerza_el_tema_opuesto() {
    let mut rt = runtime(0);
    let (x, y) = rt.find("theme").expect("el botón de tema existe");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(
        rt.state.dark_override,
        Some(false),
        "el sistema está en oscuro, así que el botón fuerza claro"
    );
}

#[test]
fn la_tabla_muestra_las_filas_cargadas() {
    let rt = runtime(42);
    assert_eq!(rt.state.total_rows(), 42);
    assert_eq!(rt.state.rows[0].code, "H-2026-0000");
    assert!(rt.presented("table-frame").is_some());
    assert!(
        rt.presented("empty-hint").is_none(),
        "con datos no debe aparecer el aviso de vacío"
    );
}

#[test]
fn una_base_vacia_muestra_un_estado_vacio_honesto() {
    let rt = runtime(0);
    assert!(rt.state.is_empty());
    assert!(rt.presented("status").is_some());
    assert!(
        rt.presented("empty-hint").is_some(),
        "sin muestras la interfaz debe explicarlo, no dejar la tabla muda"
    );
}

#[test]
fn la_barra_de_estado_refleja_el_pack_normativo() {
    let mut state = AppState::new(rows(1));
    state.norms_verified = 0;
    state.norms_pending = 3;
    state.pack_source = "archivo: laboratorio.json".into();

    let line = state.status_line();
    assert!(line.contains("3 pendientes"), "{line}");
    assert!(line.contains("laboratorio.json"), "{line}");

    let mut rt = Runtime::new(
        state,
        view,
        (1200.0, 800.0),
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    rt.on_key = Some(mamut_app::window_key);
    rt.frame(0.0);
    assert!(
        rt.presented("status-text").is_some(),
        "el texto de estado debe estar dispuesto para poder leerse"
    );
}

#[test]
fn la_paleta_no_roba_el_foco_al_cerrarse() {
    let mut rt = runtime(0);
    rt.key(chord('k'));
    rt.frame(1.0 / 60.0);
    assert!(rt.state.palette_open);

    // Elegir una sección desde la paleta también la cierra.
    let (x, y) = rt
        .find("nav-2")
        .expect("navegación accesible con la paleta abierta");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);

    assert_eq!(rt.state.nav, Nav::Concrete);
    assert!(!rt.state.palette_open, "elegir destino cierra la paleta");
}

// ------------------------------------------------- alta de muestras (fase 2) ---

fn runtime_with_store() -> Runtime<AppState> {
    let store = Store::in_memory(Timestamp::from_millis(1_763_500_000_000)).unwrap();
    let mut state = AppState::with_store(store);
    state.set_descriptor(descriptor());
    let mut rt = Runtime::new(
        state,
        view,
        (1200.0, 800.0),
        Env {
            system_dark: true,
            scale: 2.0,
        },
    );
    rt.on_key = Some(mamut_app::window_key);
    rt.frame(0.0);
    rt
}

/// Descriptor del ensayo tal como lo entrega el paquete normativo.
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

/// Registra una muestra directamente en el almacén, como haría el alta.
fn seed_sample(rt: &mut Runtime<AppState>, code: &str) {
    use mamut_core::{Actor, CivilDate};
    use mamut_domain::{Material, Sample, SampleCode};

    let sample = Sample::new(
        SampleCode::new(code).unwrap(),
        Material::Concrete,
        "Bloque 3",
        "Constructora Andina",
        "Torre Norte",
        CivilDate::new(2026, 11, 20).unwrap(),
        Actor::new("local", "Ana Torres").unwrap(),
        Timestamp::from_millis(1_763_500_000_000),
    )
    .unwrap();
    let store = rt.state.store.as_ref().expect("hay almacén");
    store
        .create_sample(
            &sample,
            &sample.responsible,
            Timestamp::from_millis(1_763_500_000_000),
        )
        .expect("la muestra de prueba se registra");
    rt.state.reload();
}

fn fill_test_form(rt: &mut Runtime<AppState>, code: &str) {
    rt.state.test_form.sample_code.text = code.to_owned();
    rt.state.test_form.designation.text = "C1".to_owned();
    rt.state.test_form.diameter.text = "150".to_owned();
    rt.state.test_form.height.text = "300".to_owned();
    rt.state.test_form.mass.text = "12300".to_owned();
    rt.state.test_form.age.text = "28".to_owned();
    rt.state.test_form.load.text = "500".to_owned();
}

/// Pulsa un elemento por su clave, como haría el usuario.
fn press(rt: &mut Runtime<AppState>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("no existe el elemento «{key}»"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
}

fn fill_valid_form(rt: &mut Runtime<AppState>, code: &str) {
    rt.state.form.code.text = code.to_owned();
    rt.state.form.origin.text = "Bloque 3".to_owned();
    rt.state.form.client.text = "Constructora Andina".to_owned();
    rt.state.form.project.text = "Torre Norte".to_owned();
    rt.state.form.received_on.text = "2026-11-20".to_owned();
    rt.state.form.responsible.text = "Ana Torres".to_owned();
}

#[test]
fn el_alta_de_una_muestra_funciona_de_punta_a_punta() {
    let mut rt = runtime_with_store();
    assert_eq!(rt.state.samples_total, 0);

    press(&mut rt, "nav-1");
    press(&mut rt, "new-sample");
    assert!(rt.state.form_open);
    assert!(
        rt.presented("sample-form").is_some(),
        "el formulario debe estar visible"
    );

    fill_valid_form(&mut rt, "H-2026-0100");
    press(&mut rt, "save-sample");

    assert!(
        rt.state.form.error.is_none(),
        "no debía haber error: {:?}",
        rt.state.form.error
    );
    assert_eq!(
        rt.state.samples_total, 1,
        "form_open={} error={:?}",
        rt.state.form_open, rt.state.form.error
    );
    assert_eq!(rt.state.rows[0].code, "H-2026-0100");
    assert!(!rt.state.form_open, "al guardar se cierra el formulario");
    assert!(
        rt.state.flash.is_some(),
        "el usuario debe saber que se guardó"
    );
}

#[test]
fn una_muestra_sin_codigo_no_se_guarda_y_el_error_se_ve() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-1");
    press(&mut rt, "new-sample");

    fill_valid_form(&mut rt, "   ");
    press(&mut rt, "save-sample");

    assert_eq!(rt.state.samples_total, 0, "no puede guardarse nada");
    assert!(rt.state.form_open, "el formulario sigue abierto");
    assert!(
        rt.presented("form-error").is_some(),
        "el error tiene que estar en pantalla, no solo en el estado"
    );
}

#[test]
fn cancelar_el_formulario_lo_cierra_sin_guardar() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-1");
    press(&mut rt, "new-sample");
    fill_valid_form(&mut rt, "H-2026-0101");

    press(&mut rt, "cancel-sample");

    assert!(!rt.state.form_open);
    assert_eq!(rt.state.samples_total, 0);
}

#[test]
fn el_material_elegido_en_el_formulario_llega_a_la_tabla() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-1");
    press(&mut rt, "new-sample");
    fill_valid_form(&mut rt, "S-2026-0001");

    press(&mut rt, "material-1");
    press(&mut rt, "save-sample");

    assert_eq!(rt.state.rows[0].material, "Suelo");
}

#[test]
fn escape_cierra_el_formulario_antes_que_la_paleta() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-1");
    press(&mut rt, "new-sample");
    assert!(rt.state.form_open);

    rt.key(escape());
    rt.frame(1.0 / 60.0);

    assert!(!rt.state.form_open, "Esc cierra el formulario");
}

// ------------------------------------------- ensayo de compresión (fase 2) ---

#[test]
fn el_ciclo_ensayo_e_informe_funciona() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0001");

    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-2026-0001");
    press(&mut rt, "calculate-test");

    assert!(
        rt.state.test_form.error.is_none(),
        "no debía fallar: {:?}",
        rt.state.test_form.error
    );
    let outcome = rt
        .state
        .test_form
        .outcome
        .clone()
        .expect("el cálculo produce resultado");
    assert!(
        (outcome.stress_mpa - 28.294_212).abs() < 1e-3,
        "500 kN sobre 150 mm dan 28.29 MPa, salió {}",
        outcome.stress_mpa
    );
    assert!(
        outcome.conformity.contains("SIN JUICIO"),
        "sin norma revisada no se dictamina: {}",
        outcome.conformity
    );
    assert!(rt.presented("test-result").is_some());

    press(&mut rt, "save-test");
    assert!(
        rt.state.test_form.error.is_none(),
        "guardar no debía fallar: {:?}",
        rt.state.test_form.error
    );
    assert!(
        rt.state.test_form.saved.is_some(),
        "el ensayo queda guardado"
    );
    assert_eq!(rt.state.test_history.len(), 1, "y aparece en el historial");

    press(&mut rt, "report-pdf");
    assert!(
        rt.state.test_form.error.is_none(),
        "el informe no debía fallar: {:?}",
        rt.state.test_form.error
    );
    let path = rt
        .state
        .test_form
        .report_path
        .clone()
        .expect("se genera un archivo");
    let bytes = std::fs::read(&path).expect("el PDF existe en disco");
    assert!(bytes.starts_with(b"%PDF"), "el archivo es un PDF");
    assert!(
        bytes.len() > 2_000,
        "y tiene contenido, {} bytes",
        bytes.len()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn no_se_guarda_un_ensayo_sin_calcular() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0002");

    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-2026-0002");
    press(&mut rt, "save-test");

    assert!(
        rt.state
            .test_form
            .error
            .as_deref()
            .unwrap_or("")
            .contains("calcula"),
        "debe pedir el cálculo primero: {:?}",
        rt.state.test_form.error
    );
    assert!(rt.state.test_form.saved.is_none());
}

#[test]
fn un_ensayo_de_una_muestra_no_registrada_no_se_guarda() {
    let mut rt = runtime_with_store();

    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-9999-0000");
    press(&mut rt, "calculate-test");
    assert!(
        rt.state.test_form.outcome.is_some(),
        "el cálculo es aritmética: no exige que la muestra exista"
    );

    press(&mut rt, "save-test");
    assert!(
        rt.state
            .test_form
            .error
            .as_deref()
            .unwrap_or("")
            .contains("no existe"),
        "al guardar sí se exige la muestra: {:?}",
        rt.state.test_form.error
    );
}

#[test]
fn una_carga_invalida_no_produce_resultado() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0003");

    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-2026-0003");
    rt.state.test_form.load.text = "0".to_owned();
    press(&mut rt, "calculate-test");

    assert!(rt.state.test_form.outcome.is_none(), "no hay resultado");
    assert!(
        rt.presented("test-error").is_some(),
        "el error debe verse en pantalla"
    );
}

#[test]
fn la_coma_decimal_se_acepta_al_escribir() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0004");

    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-2026-0004");
    rt.state.test_form.diameter.text = "150,0".to_owned();
    rt.state.test_form.load.text = "500,0".to_owned();
    press(&mut rt, "calculate-test");

    assert!(
        rt.state.test_form.error.is_none(),
        "«150,0» es un número válido en español: {:?}",
        rt.state.test_form.error
    );
}

// ------------------------------------------------- herramientas de la muestra ---

#[test]
fn las_herramientas_exportan_adjuntan_y_anulan() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0010");
    press(&mut rt, "nav-1");

    press(&mut rt, "export-csv");
    assert!(
        rt.state
            .flash
            .as_deref()
            .unwrap_or("")
            .contains("Exportado"),
        "{:?}",
        rt.state.flash
    );

    let directory = std::env::temp_dir().join(format!("mamut-tools-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("informe.pdf");
    std::fs::write(&file, b"%PDF-1.4 prueba").unwrap();

    rt.state.tool_code.text = "H-2026-0010".to_owned();
    rt.state.tool_path.text = file.display().to_string();
    press(&mut rt, "attach-file");

    assert_eq!(
        rt.state.attachments.len(),
        1,
        "el adjunto debe quedar registrado: {:?}",
        rt.state.flash
    );
    assert!(rt.presented("attachments").is_some());

    rt.state.tool_reason.text = "muestra duplicada".to_owned();
    press(&mut rt, "void-sample");

    assert!(
        rt.state.rows.iter().any(|row| row.state == "Anulada"),
        "la muestra debe quedar anulada en la tabla: {:?}",
        rt.state.flash
    );

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn dos_ensayos_dibujan_el_grafico_resistencia_edad() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0011");
    press(&mut rt, "nav-2");

    for (index, (age, load)) in [(7u32, 300.0f64), (28, 500.0)].into_iter().enumerate() {
        fill_test_form(&mut rt, "H-2026-0011");
        rt.state.test_form.designation.text = format!("C{}", index + 1);
        rt.state.test_form.age.text = age.to_string();
        rt.state.test_form.load.text = load.to_string();
        press(&mut rt, "calculate-test");
        press(&mut rt, "save-test");
        assert!(
            rt.state.test_form.error.is_none(),
            "ensayo {index}: {:?}",
            rt.state.test_form.error
        );
    }

    assert_eq!(rt.state.series.len(), 2, "{:?}", rt.state.series);
    assert!(
        rt.presented("series-chart").is_some(),
        "con dos edades distintas debe dibujarse el gráfico"
    );
}
