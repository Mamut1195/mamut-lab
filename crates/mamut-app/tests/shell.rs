#![allow(clippy::unwrap_used, clippy::expect_used)]
// Las pruebas usan unwrap/expect a propósito: si una precondición falla,
// queremos que la prueba estalle justo ahí.

//! Pruebas del armazón de la aplicación, **sin ventana**.
//!
//! Se ejecutan con el reloj virtual de Rune: el mismo código que dibuja en pantalla se
//! monta en memoria, se localizan elementos por su clave y se simula entrada real.

use mamut_app::{AppState, Nav, SampleRow, view};
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
