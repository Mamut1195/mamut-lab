//! Punto de entrada: composición de dependencias.
//!
//! Aquí (y solo aquí) se juntan disco, base de datos, pack normativo e interfaz. La vista
//! recibe un estado ya construido, de modo que un fallo de almacenamiento se muestra como
//! un aviso en la barra inferior en lugar de impedir abrir la aplicación.

use std::path::PathBuf;

use mamut_app::{AppState, Store, load_pack};
use mamut_core::{Clock, SystemClock};
use rune::AppSpec;

fn main() {
    // `AppSpec::new` exige un puntero a función: el arranque resuelve las rutas él mismo.
    // Los atajos van a nivel de ventana (`on_key`), no de elemento: si estuvieran en un
    // elemento, solo funcionarían cuando ese elemento tuviera el foco.
    rune::run(AppSpec {
        on_key: Some(mamut_app::window_key),
        // Escenas para revisión de diseño sin datos:
        //   --scene alta     formulario de muestra
        //   --scene ensayo   ensayo calculado (aritmética, sin base de datos)
        scenes: &[
            ("alta", |s: &mut AppState| {
                s.select(mamut_app::Nav::Samples);
                s.open_form();
            }),
            ("ensayo", |s: &mut AppState| {
                s.select(mamut_app::Nav::Concrete);
                s.test_form.sample_code.text = "H-2026-0001".to_owned();
                s.test_form.mass.text = "12300".to_owned();
                s.test_form.load.text = "500".to_owned();
                s.calculate_test();
            }),
        ],
        ..AppSpec::new("MAMUT Lab", (1200.0, 800.0), startup, mamut_app::view)
    });
}

/// Carpeta de datos del usuario según la plataforma.
fn data_dir() -> PathBuf {
    let base = rune::dirs::AppDirs::new("com.mamutlab.desktop")
        .map(|dirs| dirs.data().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    if let Err(error) = std::fs::create_dir_all(&base) {
        eprintln!("no se pudo crear la carpeta de datos {base:?}: {error}");
    }
    base
}

fn startup() -> AppState {
    let data_dir = data_dir();
    let db_path = data_dir.join("mamut.db");
    let mut notes: Vec<String> = Vec::new();

    // Almacenamiento. Si falla, la aplicación abre igualmente y lo dice: es preferible
    // poder ver el estado del sistema a una ventana que no aparece.
    let mut state = match Store::open(&db_path, SystemClock.now()) {
        Ok(store) => {
            notes.push("base local".to_owned());
            AppState::with_store(store)
        }
        Err(error) => {
            notes.push(format!("almacenamiento no disponible: {error}"));
            AppState::new(Vec::new())
        }
    };

    // Pack normativo: del usuario si lo hay, embebido en caso contrario.
    match load_pack(Some(&data_dir)) {
        Ok(selection) => {
            let problems = selection.issues.len();
            state.norms_verified = selection.pack.verified_count();
            state.norms_pending = selection
                .pack
                .pack()
                .norms
                .len()
                .saturating_sub(state.norms_verified);
            // Descriptor del ensayo de compresión, con su norma del pack.
            if let Some(descriptor) = selection.pack.descriptors().into_iter().next() {
                state.set_descriptor(descriptor);
            }
            state.pack_source = selection.source;
            if problems > 0 {
                notes.push(format!(
                    "pack del usuario descartado: {problems} problema(s)"
                ));
            }
        }
        Err(issues) => {
            notes.push(format!(
                "pack normativo embebido inválido: {} problema(s)",
                issues.len()
            ));
        }
    }

    state.loaded_at = SystemClock.now();
    state.storage_status = notes.join(" · ");
    state
}
