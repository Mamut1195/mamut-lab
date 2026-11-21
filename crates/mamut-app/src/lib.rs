//! `mamut-app` — la única capa que conoce Rune.
//!
//! Todo lo demás (dominio, cálculos, normas, almacenamiento) se compila y se prueba sin
//! el framework, de modo que sustituirlo no obligue a reescribir el producto (ADR-0008).
//!
//! La vista es una función del estado: `fn view(&AppState, &mut Cx<AppState>) -> El`.
//! Ninguna consulta a la base de datos ocurre aquí: el estado se construye antes de
//! arrancar y las escrituras pasan por [`Store`].

pub mod pack;
pub mod state;
pub mod store;
pub mod view;

pub use pack::{EMBEDDED_PACK, PackSelection, load_pack};
pub use state::{AppState, CompactionPointView, Nav, SampleRow};
pub use store::Store;
pub use view::{handle_key, view, window_key};
