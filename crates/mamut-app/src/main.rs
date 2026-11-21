//! Punto de entrada: composición de dependencias.
//!
//! Aquí (y solo aquí) se juntan disco, base de datos, pack normativo e interfaz. La vista
//! recibe un estado ya construido, de modo que un fallo de almacenamiento se muestra como
//! un aviso en la barra inferior en lugar de impedir abrir la aplicación.

use std::path::PathBuf;

use mamut_app::{AppState, SampleRow, load_pack, load_sample_rows};
use mamut_core::{Clock, SystemClock};
use mamut_storage::Database;
use rune::AppSpec;

/// Cuántas muestras se cargan en la tabla al arrancar.
const SAMPLE_LIMIT: usize = 500;

fn main() {
    // `AppSpec::new` exige un puntero a función: el arranque resuelve las rutas él mismo.
    // Los atajos van a nivel de ventana (`on_key`), no de elemento: si estuvieran en un
    // elemento, solo funcionarían cuando ese elemento tuviera el foco.
    rune::run(AppSpec {
        on_key: Some(mamut_app::window_key),
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

    let rows: Vec<SampleRow> = match open_and_load(&db_path) {
        Ok(rows) => {
            // La ruta completa no cabe en la barra de estado; el detalle va al registro.
            notes.push("base local".to_owned());
            rows
        }
        Err(error) => {
            // Un fallo de almacenamiento no debe impedir abrir la aplicación: se informa.
            notes.push(format!("almacenamiento no disponible: {error}"));
            Vec::new()
        }
    };

    let mut verified = 0;
    let mut pending = 0;
    let mut pack_source = "no cargado".to_owned();

    match load_pack(Some(&data_dir)) {
        Ok(selection) => {
            verified = selection.pack.verified_count();
            pending = selection.pack.pack().norms.len().saturating_sub(verified);
            let problems = selection.issues.len();
            pack_source = selection.source;
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

    let mut state = AppState::new(rows);
    state.norms_verified = verified;
    state.norms_pending = pending;
    state.pack_source = pack_source;
    state.loaded_at = SystemClock.now();
    state.storage_status = notes.join(" · ");
    state
}

fn open_and_load(path: &std::path::Path) -> Result<Vec<SampleRow>, String> {
    let mut db = Database::open(path).map_err(|error| error.to_string())?;
    db.migrate(SystemClock.now())
        .map_err(|error| error.to_string())?;
    load_sample_rows(&db, SAMPLE_LIMIT).map_err(|error| error.to_string())
}
