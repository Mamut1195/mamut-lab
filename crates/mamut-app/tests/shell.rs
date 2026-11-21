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

// ------------------------------------------------------- granulometría (fase 3) ---

#[test]
fn la_granulometria_se_calcula_en_la_interfaz() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");

    rt.state.soils.total.text = "1000".to_owned();
    // Serie por defecto: 9,5 · 4,75 · 2,0 · 0,425 · 0,075 mm.
    for (index, value) in ["0", "100", "200", "300", "200"].iter().enumerate() {
        rt.state.soils.rows[index].retained.text = (*value).to_owned();
    }

    press(&mut rt, "calculate-granulometry");

    assert!(
        rt.state.soils.error.is_none(),
        "no debía fallar: {:?}",
        rt.state.soils.error
    );
    let outcome = rt.state.soils.outcome.clone().expect("resultado");
    assert_eq!(outcome.passing.len(), 5);
    // 1000 g con 800 g retenidos: el 20 % pasa el tamiz de 0,075 mm.
    assert!((outcome.passing[4].1 - 20.0).abs() < 1e-9);
    assert!(rt.presented("soils-result").is_some());
}

#[test]
fn una_fila_con_retenido_pero_sin_abertura_es_un_error() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");

    rt.state.soils.total.text = "1000".to_owned();
    rt.state.soils.rows[0].retained.text = "100".to_owned();
    // Retenido sin tamiz: falta el dato que da sentido a la masa.
    rt.state.soils.rows[1].retained.text = "50".to_owned();
    rt.state.soils.rows[1].size.text = String::new();

    press(&mut rt, "calculate-granulometry");

    assert!(rt.state.soils.outcome.is_none());
    assert!(
        rt.state
            .soils
            .error
            .as_deref()
            .unwrap_or("")
            .contains("abertura"),
        "{:?}",
        rt.state.soils.error
    );
    assert!(rt.presented("soils-error").is_some(), "y se ve en pantalla");
}

#[test]
fn sin_masa_total_la_granulometria_no_calcula() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    rt.state.soils.rows[0].retained.text = "100".to_owned();
    press(&mut rt, "calculate-granulometry");

    assert!(rt.state.soils.outcome.is_none());
    assert!(
        rt.state
            .soils
            .error
            .as_deref()
            .unwrap_or("")
            .contains("masa seca total"),
        "{:?}",
        rt.state.soils.error
    );
}

#[test]
fn se_puede_añadir_un_tamiz_mas() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    let before = rt.state.soils.rows.len();
    press(&mut rt, "add-sieve");
    assert_eq!(rt.state.soils.rows.len(), before + 1);
}

#[test]
fn los_limites_de_atterberg_se_calculan() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");

    // Tres determinaciones coherentes: más golpes, menos humedad.
    for (index, (blows, moisture)) in [("20", "45"), ("30", "41"), ("40", "38")]
        .iter()
        .enumerate()
    {
        rt.state.limits.rows[index].blows.text = (*blows).to_owned();
        rt.state.limits.rows[index].moisture.text = (*moisture).to_owned();
    }
    // Límite plástico: agua 5 g, seca 25 g, tara 5 g → 25 %.
    rt.state.limits.water.text = "5".to_owned();
    rt.state.limits.dry.text = "25".to_owned();
    rt.state.limits.tare.text = "5".to_owned();

    rt.state.limits_calculate();

    assert!(
        rt.state.limits.error.is_none(),
        "no debía fallar: {:?}",
        rt.state.limits.error
    );
    let outcome = rt.state.limits.outcome.clone().expect("resultado");
    assert!((41.0..45.0).contains(&outcome.liquid_limit));
    assert_eq!(outcome.determinations, 3);
    assert!(outcome.slope < 0.0);
    let plastic = outcome.plastic_limit.expect("límite plástico");
    assert!((plastic - 25.0).abs() < 1e-9);
    let index = outcome.plasticity_index.expect("índice");
    assert!((index - (outcome.liquid_limit - 25.0)).abs() < 1e-9);
}

#[test]
fn con_una_sola_determinacion_no_hay_limite_liquido() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    rt.state.limits.rows[0].blows.text = "25".to_owned();
    rt.state.limits.rows[0].moisture.text = "40".to_owned();

    rt.state.limits_calculate();

    assert!(rt.state.limits.outcome.is_none());
    assert!(
        rt.state
            .limits
            .error
            .as_deref()
            .unwrap_or("")
            .contains("tres"),
        "{:?}",
        rt.state.limits.error
    );
}

#[test]
fn el_limite_plastico_a_medias_es_un_error() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    for (index, (blows, moisture)) in [("20", "45"), ("30", "41"), ("40", "38")]
        .iter()
        .enumerate()
    {
        rt.state.limits.rows[index].blows.text = (*blows).to_owned();
        rt.state.limits.rows[index].moisture.text = (*moisture).to_owned();
    }
    rt.state.limits.water.text = "5".to_owned();
    // Falta la masa seca y la tara.

    rt.state.limits_calculate();

    assert!(rt.state.limits.outcome.is_none());
    assert!(
        rt.state
            .limits
            .error
            .as_deref()
            .unwrap_or("")
            .contains("tres masas"),
        "{:?}",
        rt.state.limits.error
    );
}

#[test]
fn la_densidad_en_sitio_se_calcula_y_avisa() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");

    // 2,145 g/cm³ húmeda al 10 % → 1,950 g/cm³ seca, que es exactamente la máxima.
    rt.state.density.wet.text = "2.145".to_owned();
    rt.state.density.moisture.text = "10".to_owned();
    rt.state.density.max_dry.text = "1.95".to_owned();
    rt.state.density.optimum.text = "10".to_owned();

    rt.state.density_calculate();

    assert!(
        rt.state.density.error.is_none(),
        "{:?}",
        rt.state.density.error
    );
    let outcome = rt.state.density.outcome.clone().expect("resultado");
    assert!((outcome.relative_compaction - 100.0).abs() < 1e-6);
    assert!(outcome.warnings.is_empty());

    // Una densidad húmeda imposible: 117 % de compactación, con aviso.
    rt.state.density.wet.text = "2.40".to_owned();
    rt.state.density.moisture.text = "5".to_owned();
    rt.state.density_calculate();
    let outcome = rt.state.density.outcome.clone().expect("resultado");
    assert!(outcome.relative_compaction > 100.0);
    assert!(!outcome.warnings.is_empty());
}

#[test]
fn sin_densidad_maxima_no_hay_compactacion() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    rt.state.density.wet.text = "2.10".to_owned();
    rt.state.density.moisture.text = "10".to_owned();
    rt.state.density.optimum.text = "11".to_owned();

    rt.state.density_calculate();

    assert!(rt.state.density.outcome.is_none());
    assert!(rt.state.density.error.is_some());
}

#[test]
fn un_punto_de_compactacion_se_calcula_desde_las_masas() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");

    rt.state.compaction.mold_plus.text = "5000".to_owned();
    rt.state.compaction.mold.text = "3000".to_owned();
    rt.state.compaction.volume.text = "943".to_owned();
    rt.state.compaction.wet_sample.text = "220".to_owned();
    rt.state.compaction.dry_sample.text = "200".to_owned();

    rt.state.compaction_add_point();

    assert!(
        rt.state.compaction.error.is_none(),
        "{:?}",
        rt.state.compaction.error
    );
    let point = rt.state.compaction.points[0];
    assert!((point.moisture - 10.0).abs() < 1e-9, "{}", point.moisture);
    // 2000 g / 943 cm³ = 2,1209 g/cm³ húmeda; seca = 2,1209 / 1,10 = 1,9281.
    assert!(
        (point.dry_density - 1.928_083).abs() < 1e-5,
        "{}",
        point.dry_density
    );
    // Las masas de la muestra se limpian para el siguiente punto; el molde no.
    assert!(rt.state.compaction.wet_sample.text.is_empty());
    assert_eq!(rt.state.compaction.mold.text, "3000");
}

#[test]
fn tres_puntos_definen_el_maximo_de_la_curva() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    rt.state.compaction.mold.text = "3000".to_owned();
    rt.state.compaction.volume.text = "943".to_owned();

    // Tres puntos con humedades 8, 10 y 12 % y densidades secas 1,85 / 1,95 / 1,90.
    for (mold_plus, wet, dry) in [
        ("4884.1", "216", "200"),
        ("5022.7", "220", "200"),
        ("5006.7", "224", "200"),
    ] {
        rt.state.compaction.mold_plus.text = mold_plus.to_owned();
        rt.state.compaction.wet_sample.text = wet.to_owned();
        rt.state.compaction.dry_sample.text = dry.to_owned();
        rt.state.compaction_add_point();
        assert!(
            rt.state.compaction.error.is_none(),
            "{:?}",
            rt.state.compaction.error
        );
    }
    assert_eq!(rt.state.compaction.points.len(), 3);

    rt.state.compaction_calculate();

    let peak = rt.state.compaction.peak.expect("hay máximo");
    assert!(
        (peak.optimum_moisture - 10.33).abs() < 0.1,
        "{}",
        peak.optimum_moisture
    );
    assert!(peak.max_dry_density > 1.94 && peak.max_dry_density < 1.96);
    assert_eq!(peak.points_used, 3);
}

#[test]
fn con_dos_puntos_no_hay_maximo_de_compactacion() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    rt.state
        .compaction
        .points
        .push(mamut_app::CompactionPointView {
            mold_g: 3000.0,
            mold_plus_g: 5000.0,
            volume_cm3: 943.0,
            wet_sample_g: 220.0,
            dry_sample_g: 200.0,
            moisture: 8.0,
            dry_density: 1.85,
        });
    rt.state
        .compaction
        .points
        .push(mamut_app::CompactionPointView {
            mold_g: 3000.0,
            mold_plus_g: 5200.0,
            volume_cm3: 943.0,
            wet_sample_g: 220.0,
            dry_sample_g: 200.0,
            moisture: 10.0,
            dry_density: 1.95,
        });

    rt.state.compaction_calculate();

    assert!(rt.state.compaction.peak.is_none());
    assert!(
        rt.state
            .compaction
            .error
            .as_deref()
            .unwrap_or("")
            .contains("tres"),
        "{:?}",
        rt.state.compaction.error
    );
}

#[test]
fn el_cbr_se_calcula_con_la_penetracion_que_gobierna() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");

    // Curva lineal: 3,4474 MPa a 2,54 mm y 5,1711 MPa a 5,08 mm → 50 %.
    rt.state.cbr.rows[0].load.text = "0".to_owned();
    rt.state.cbr.rows[3].load.text = "3.4474".to_owned();
    rt.state.cbr.rows[4].load.text = "5.1711".to_owned();

    rt.state.cbr_calculate();

    assert!(rt.state.cbr.error.is_none(), "{:?}", rt.state.cbr.error);
    let outcome = rt.state.cbr.outcome.expect("resultado");
    assert!((outcome.percent - 50.0).abs() < 0.01, "{}", outcome.percent);
    assert!((outcome.governing_penetration - 2.54).abs() < 1e-9);
    assert!(outcome.offset.abs() < 1e-9);
}

#[test]
fn un_cbr_sin_curva_suficiente_no_se_calcula() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    rt.state.cbr.rows[3].load.text = "3.4474".to_owned();

    rt.state.cbr_calculate();

    assert!(rt.state.cbr.outcome.is_none());
    assert!(rt.state.cbr.error.is_some());
}

#[test]
fn los_cuatro_informes_de_suelos_se_generan() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "S-2026-0001");
    press(&mut rt, "nav-3");
    rt.state.soils.sample.text = "S-2026-0001".to_owned();

    // Atterberg
    for (index, (blows, moisture)) in [("20", "45"), ("30", "41"), ("40", "38")]
        .iter()
        .enumerate()
    {
        rt.state.limits.rows[index].blows.text = (*blows).to_owned();
        rt.state.limits.rows[index].moisture.text = (*moisture).to_owned();
    }
    rt.state.limits_calculate();
    rt.state.report_atterberg();
    assert!(
        rt.state.limits.error.is_none(),
        "{:?}",
        rt.state.limits.error
    );

    // Densidad en sitio
    rt.state.density.wet.text = "2.145".to_owned();
    rt.state.density.moisture.text = "10".to_owned();
    rt.state.density.max_dry.text = "1.95".to_owned();
    rt.state.density.optimum.text = "10".to_owned();
    rt.state.density_calculate();
    rt.state.report_density();
    assert!(
        rt.state.density.error.is_none(),
        "{:?}",
        rt.state.density.error
    );

    // Compactación: tres puntos
    rt.state.compaction.mold.text = "3000".to_owned();
    rt.state.compaction.volume.text = "943".to_owned();
    for (mold_plus, wet, dry) in [
        ("4884.1", "216", "200"),
        ("5022.7", "220", "200"),
        ("5006.7", "224", "200"),
    ] {
        rt.state.compaction.mold_plus.text = mold_plus.to_owned();
        rt.state.compaction.wet_sample.text = wet.to_owned();
        rt.state.compaction.dry_sample.text = dry.to_owned();
        rt.state.compaction_add_point();
    }
    rt.state.compaction_calculate();
    rt.state.report_compaction();
    assert!(
        rt.state.compaction.error.is_none(),
        "{:?}",
        rt.state.compaction.error
    );

    // CBR
    rt.state.cbr.rows[0].load.text = "0".to_owned();
    rt.state.cbr.rows[3].load.text = "3.4474".to_owned();
    rt.state.cbr.rows[4].load.text = "5.1711".to_owned();
    rt.state.cbr_calculate();
    rt.state.report_cbr();
    assert!(rt.state.cbr.error.is_none(), "{:?}", rt.state.cbr.error);

    let directory = rt
        .state
        .store
        .as_ref()
        .expect("almacén")
        .data_dir()
        .join("reports");
    for stem in ["atterberg", "densidad", "compactacion", "cbr"] {
        let path = directory.join(format!("{stem}-S-2026-0001.pdf"));
        let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("falta el informe {stem}"));
        assert!(bytes.starts_with(b"%PDF"), "{stem} no es un PDF");
        assert!(bytes.len() > 2_000, "{stem} está vacío");
        let _ = std::fs::remove_file(&path);
    }

    // Cada informe deja su ensayo y su registro con hash.
    let store = rt.state.store.as_ref().expect("almacén");
    let tests = store.soil_tests_for("S-2026-0001").unwrap();
    assert_eq!(tests.len(), 4, "cuatro ensayos de suelos guardados");
    let reports = store.reports_for("S-2026-0001").unwrap();
    assert_eq!(reports.len(), 4, "cuatro informes registrados");
    for report in &reports {
        assert_eq!(report.sha256.len(), 64);
        assert!(report.path.ends_with(".pdf"));
    }
}

#[test]
fn sin_muestra_no_se_emite_informe_de_suelos() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");

    for (index, (blows, moisture)) in [("20", "45"), ("30", "41"), ("40", "38")]
        .iter()
        .enumerate()
    {
        rt.state.limits.rows[index].blows.text = (*blows).to_owned();
        rt.state.limits.rows[index].moisture.text = (*moisture).to_owned();
    }
    rt.state.limits_calculate();
    // Sin código de muestra.
    rt.state.report_atterberg();

    assert!(rt.state.limits.error.is_some());
    assert!(
        rt.state
            .limits
            .error
            .as_deref()
            .unwrap_or("")
            .contains("muestra"),
        "{:?}",
        rt.state.limits.error
    );
}

#[test]
fn el_informe_granulometrico_se_genera_desde_la_interfaz() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "S-2026-0001");
    press(&mut rt, "nav-3");

    rt.state.soils.sample.text = "S-2026-0001".to_owned();
    rt.state.soils.total.text = "1000".to_owned();
    for (index, value) in ["0", "100", "200", "300", "200"].iter().enumerate() {
        rt.state.soils.rows[index].retained.text = (*value).to_owned();
    }

    press(&mut rt, "calculate-granulometry");
    press(&mut rt, "granulometry-pdf");

    assert!(
        rt.state.soils.error.is_none(),
        "no debía fallar: {:?}",
        rt.state.soils.error
    );
    let path = rt
        .state
        .soils
        .report_path
        .clone()
        .expect("se genera un archivo");
    let bytes = std::fs::read(&path).expect("el PDF existe en disco");
    assert!(bytes.starts_with(b"%PDF"));

    // El ensayo y el informe quedan registrados: el PDF no es la única evidencia.
    let store = rt.state.store.as_ref().expect("almacén");
    assert_eq!(
        store.soil_tests_for("S-2026-0001").unwrap().len(),
        1,
        "el ensayo de granulometría debe guardarse"
    );
    let reports = store.reports_for("S-2026-0001").unwrap();
    assert_eq!(reports.len(), 1, "el informe debe quedar registrado");
    assert_eq!(reports[0].sha256.len(), 64);

    let _ = std::fs::remove_file(path);
}

#[test]
fn sin_muestra_el_informe_granulometrico_no_se_emite() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    rt.state.soils.total.text = "1000".to_owned();
    rt.state.soils.rows[0].retained.text = "100".to_owned();

    press(&mut rt, "calculate-granulometry");
    press(&mut rt, "granulometry-pdf");

    assert!(rt.state.soils.report_path.is_none());
    assert!(
        rt.state
            .soils
            .error
            .as_deref()
            .unwrap_or("")
            .contains("muestra"),
        "{:?}",
        rt.state.soils.error
    );
}

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
            .contains("Exportadas"),
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
    // Anular no se deshace: el primer clic pide confirmación y no anula nada.
    press(&mut rt, "void-sample");
    assert!(rt.state.pending_void.is_some(), "debe pedir confirmación");
    assert!(
        !rt.state.rows.iter().any(|row| row.state == "Anulada"),
        "el primer clic no anula"
    );

    // El segundo clic, sobre la misma muestra, sí anula.
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

#[test]
fn editar_despues_de_calcular_invalida_el_ensayo() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0020");
    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-2026-0020");
    press(&mut rt, "calculate-test");
    assert!(rt.state.test_form.outcome.is_some());

    // Se cambia la carga después de calcular: el resultado ya no corresponde.
    rt.state.test_form.load.text = "600".to_owned();
    press(&mut rt, "save-test");

    assert!(rt.state.test_form.saved.is_none(), "no se guarda");
    assert!(
        rt.state
            .test_form
            .error
            .as_deref()
            .unwrap_or("")
            .contains("cambiaron"),
        "{:?}",
        rt.state.test_form.error
    );
}

#[test]
fn editar_despues_de_calcular_invalida_la_granulometria() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    rt.state.soils.sample.text = "S-2026-0030".to_owned();
    rt.state.soils.total.text = "1000".to_owned();
    for (index, value) in ["0", "100", "200", "300", "200"].iter().enumerate() {
        rt.state.soils.rows[index].retained.text = (*value).to_owned();
    }
    press(&mut rt, "calculate-granulometry");
    assert!(rt.state.soils.outcome.is_some());

    // Se cambia la masa total después de calcular.
    rt.state.soils.total.text = "1200".to_owned();
    press(&mut rt, "granulometry-pdf");

    assert!(
        rt.state.soils.report_path.is_none(),
        "no se emite el informe"
    );
    assert!(
        rt.state
            .soils
            .error
            .as_deref()
            .unwrap_or("")
            .contains("cambiaron"),
        "{:?}",
        rt.state.soils.error
    );
}

#[test]
fn un_ensayo_con_coma_decimal_se_puede_guardar() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0040");
    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-2026-0040");
    rt.state.test_form.diameter.text = "150,0".to_owned();
    rt.state.test_form.load.text = "500,0".to_owned();

    press(&mut rt, "calculate-test");
    press(&mut rt, "save-test");

    assert!(
        rt.state.test_form.error.is_none(),
        "calcular y guardar deben aceptar la misma coma: {:?}",
        rt.state.test_form.error
    );
    assert!(rt.state.test_form.saved.is_some());
}

#[test]
fn una_muestra_anulada_no_acepta_ensayos() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0041");

    let store = rt.state.store.as_ref().expect("almacén");
    let actor = mamut_core::Actor::new("local", "Ana Torres").unwrap();
    store
        .void_sample(
            "H-2026-0041",
            &actor,
            Timestamp::from_millis(1_763_500_000_000),
            "duplicada",
        )
        .unwrap();

    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-2026-0041");
    press(&mut rt, "calculate-test");
    press(&mut rt, "save-test");

    assert!(rt.state.test_form.saved.is_none(), "no se guarda");
    assert!(
        rt.state
            .test_form
            .error
            .as_deref()
            .unwrap_or("")
            .contains("no acepta ensayos"),
        "{:?}",
        rt.state.test_form.error
    );
}

#[test]
fn un_informe_de_una_muestra_no_registrada_se_rechaza() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-3");
    rt.state.soils.sample.text = "S-2026-9999".to_owned();
    rt.state.soils.total.text = "1000".to_owned();
    for (index, value) in ["0", "100", "200", "300", "200"].iter().enumerate() {
        rt.state.soils.rows[index].retained.text = (*value).to_owned();
    }

    press(&mut rt, "calculate-granulometry");
    press(&mut rt, "granulometry-pdf");

    assert!(rt.state.soils.report_path.is_none());
    assert!(
        rt.state
            .soils
            .error
            .as_deref()
            .unwrap_or("")
            .contains("no existe"),
        "{:?}",
        rt.state.soils.error
    );
}

#[test]
fn el_informe_de_compresion_registra_el_ensayo_y_lo_marca_informado() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0050");
    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-2026-0050");

    press(&mut rt, "calculate-test");
    press(&mut rt, "save-test");
    assert!(
        rt.state.test_form.error.is_none(),
        "{:?}",
        rt.state.test_form.error
    );

    press(&mut rt, "report-pdf");
    assert!(
        rt.state.test_form.error.is_none(),
        "el informe no debía fallar: {:?}",
        rt.state.test_form.error
    );

    let store = rt.state.store.as_ref().expect("almacén");
    let reports = store.reports_for("H-2026-0050").unwrap();
    assert_eq!(reports.len(), 1, "el informe queda registrado");
    assert_eq!(reports[0].kind, "compresion");
    assert_eq!(reports[0].sha256.len(), 64);

    let tests = store.tests_for_sample("H-2026-0050").unwrap();
    assert_eq!(tests.len(), 1);
    assert_eq!(
        tests[0].state,
        mamut_domain::TestState::Reported,
        "el ensayo pasa a informado"
    );
}

#[test]
fn la_seccion_de_informes_lista_los_emitidos() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "H-2026-0060");
    press(&mut rt, "nav-2");
    fill_test_form(&mut rt, "H-2026-0060");
    press(&mut rt, "calculate-test");
    press(&mut rt, "save-test");
    press(&mut rt, "report-pdf");
    assert!(
        rt.state.test_form.error.is_none(),
        "el informe debía emitirse: {:?}",
        rt.state.test_form.error
    );

    press(&mut rt, "nav-5");
    rt.state.reports_code.text = "H-2026-0060".to_owned();
    press(&mut rt, "load-reports");

    assert_eq!(
        rt.state.reports_list.len(),
        1,
        "un informe emitido: {:?}",
        rt.state.reports_error
    );
    assert!(rt.presented("reports-list").is_some());
    assert!(rt.state.reports_list[0].contains("compresion"));
}

#[test]
fn sin_muestra_la_seccion_de_informes_avisa() {
    let mut rt = runtime_with_store();
    press(&mut rt, "nav-5");
    press(&mut rt, "load-reports");

    assert!(rt.state.reports_list.is_empty());
    assert!(
        rt.state
            .reports_error
            .as_deref()
            .unwrap_or("")
            .contains("muestra"),
        "{:?}",
        rt.state.reports_error
    );
}

#[test]
fn cambiar_el_volumen_tras_los_puntos_no_altera_el_informe() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "S-2026-0070");
    press(&mut rt, "nav-3");
    rt.state.soils.sample.text = "S-2026-0070".to_owned();

    rt.state.compaction.mold.text = "3000".to_owned();
    rt.state.compaction.volume.text = "943".to_owned();
    for (mold_plus, wet, dry) in [
        ("4884.1", "216", "200"),
        ("5022.7", "220", "200"),
        ("5006.7", "224", "200"),
    ] {
        rt.state.compaction.mold_plus.text = mold_plus.to_owned();
        rt.state.compaction.wet_sample.text = wet.to_owned();
        rt.state.compaction.dry_sample.text = dry.to_owned();
        rt.state.compaction_add_point();
    }
    rt.state.compaction_calculate();
    let peak_before = rt.state.compaction.peak.clone().expect("máximo");

    // El volumen se cambia después: los puntos ya tienen el suyo y el informe los usa.
    rt.state.compaction.volume.text = "2000".to_owned();
    rt.state.report_compaction();

    assert!(
        rt.state.compaction.error.is_none(),
        "el informe no debía fallar: {:?}",
        rt.state.compaction.error
    );
    assert_eq!(
        rt.state
            .compaction
            .peak
            .as_ref()
            .map(|peak| peak.max_dry_density),
        Some(peak_before.max_dry_density),
        "el máximo no cambia por editar el texto"
    );
}

#[test]
fn tocar_las_pesadas_tras_calcular_bloquea_el_informe() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "S-2026-0071");
    press(&mut rt, "nav-3");
    rt.state.soils.sample.text = "S-2026-0071".to_owned();

    rt.state.compaction.mold.text = "3000".to_owned();
    rt.state.compaction.volume.text = "943".to_owned();
    for (mold_plus, wet, dry) in [
        ("4884.1", "216", "200"),
        ("5022.7", "220", "200"),
        ("5006.7", "224", "200"),
    ] {
        rt.state.compaction.mold_plus.text = mold_plus.to_owned();
        rt.state.compaction.wet_sample.text = wet.to_owned();
        rt.state.compaction.dry_sample.text = dry.to_owned();
        rt.state.compaction_add_point();
    }
    rt.state.compaction_calculate();

    // Se altera una pesada de un punto ya calculado.
    rt.state.compaction.points[0].volume_cm3 = 500.0;
    rt.state.report_compaction();

    assert!(
        rt.state
            .compaction
            .error
            .as_deref()
            .unwrap_or("")
            .contains("cambiaron"),
        "{:?}",
        rt.state.compaction.error
    );
}

#[test]
fn la_granulometria_sin_d60_no_guarda_un_diametro_imposible() {
    let mut rt = runtime_with_store();
    seed_sample(&mut rt, "S-2026-0072");
    press(&mut rt, "nav-3");
    rt.state.soils.sample.text = "S-2026-0072".to_owned();
    rt.state.soils.total.text = "1000".to_owned();
    // Pasa 99, 98, 97, 96 y 95 %: nunca alcanza el 60 %.
    for (index, value) in ["10", "10", "10", "10", "10"].iter().enumerate() {
        rt.state.soils.rows[index].retained.text = (*value).to_owned();
    }
    press(&mut rt, "calculate-granulometry");
    assert!(rt.state.soils.outcome.is_some());

    press(&mut rt, "granulometry-pdf");
    assert!(rt.state.soils.error.is_none(), "{:?}", rt.state.soils.error);

    let store = rt.state.store.as_ref().expect("almacén");
    let tests = store.soil_tests_for("S-2026-0072").unwrap();
    assert_eq!(tests.len(), 1);
    assert!(
        !tests[0].result_unit.contains("mm"),
        "sin D60 no se guarda un diámetro: {}",
        tests[0].result_unit
    );
    assert!(tests[0].result_value > 0.0, "y el valor no es cero");
}
