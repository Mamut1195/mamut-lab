//! `mamut-app` — la única capa que conoce Rune.
//!
//! Todo lo demás (dominio, cálculos, normas, almacenamiento) se compila y se prueba sin
//! el framework, de modo que sustituirlo no obligue a reescribir el producto (ADR-0008).
//!
//! La vista es una función del estado: `fn view(&AppState, &mut Cx<AppState>) -> El`.
//! Ninguna consulta a la base de datos ocurre aquí; las filas se cargan antes de arrancar
//! y la vista solo las presenta.

pub mod pack;
pub mod state;
pub mod view;

pub use pack::{EMBEDDED_PACK, PackSelection, load_pack};
pub use state::{AppState, Nav, SampleRow, load_sample_rows};
pub use view::{handle_key, view, window_key};
