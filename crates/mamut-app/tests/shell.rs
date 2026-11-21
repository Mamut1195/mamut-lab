#![allow(clippy::unwrap_used, clippy::expect_used)]
// Las pruebas usan unwrap/expect a propósito: si una precondición falla,
// queremos que la prueba estalle justo ahí.

//! Pruebas del armazón de la aplicación, **sin ventana**.
//!
//! Se ejecutan con el reloj virtual de Rune: el mismo código que dibuja en pantalla se
//! monta en memoria, se localizan elementos por su clave y se simula entrada real.

use mamut_app::{AppState, Nav, SampleRow, Store, view};
use mamut_core::Timestamp;
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
    let mut rt = Runtime::new(
        AppState::with_store(store),
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
