//! Estado de la interfaz.
//!
//! Es **solo** lo que la pantalla necesita para dibujarse: sección activa, filas que se
//! muestran, tema, paleta. No contiene reglas de negocio ni acceso a datos.

use std::cell::Cell;

use mamut_core::Timestamp;
use mamut_domain::Sample;
use mamut_storage::{Database, StorageError};
use rune_kit::TableState;

/// Secciones de la navegación lateral.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Dashboard,
    Samples,
    Concrete,
    Soils,
    Equipment,
    Reports,
}

impl Nav {
    pub const ALL: [Nav; 6] = [
        Nav::Dashboard,
        Nav::Samples,
        Nav::Concrete,
        Nav::Soils,
        Nav::Equipment,
        Nav::Reports,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Tablero",
            Self::Samples => "Muestras",
            Self::Concrete => "Hormigón",
            Self::Soils => "Suelos",
            Self::Equipment => "Equipos",
            Self::Reports => "Informes",
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|n| *n == self).unwrap_or(0)
    }
}

/// Una fila de la tabla de muestras, ya formateada para mostrar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleRow {
    pub code: String,
    pub material: String,
    pub client: String,
    pub received_on: String,
    pub state: String,
}

impl SampleRow {
    pub fn from_sample(sample: &Sample) -> Self {
        Self {
            code: sample.code.as_str().to_owned(),
            material: sample.material.label().to_owned(),
            client: sample.client.clone(),
            received_on: sample.received_on.to_iso(),
            state: sample.state.label().to_owned(),
        }
    }
}

/// Carga las muestras más recientes para la tabla. Es la única lectura de datos que hace
/// la aplicación al arrancar.
pub fn load_sample_rows(db: &Database, limit: usize) -> Result<Vec<SampleRow>, StorageError> {
    let samples = db.samples().list(limit)?;
    Ok(samples.iter().map(SampleRow::from_sample).collect())
}

pub struct AppState {
    pub nav: Nav,
    pub rows: Vec<SampleRow>,
    pub table: TableState,
    /// `None` = seguir el tema del sistema.
    pub dark_override: Option<bool>,
    /// Caché del tema del sistema, para que el botón pueda invertirlo.
    system_dark: Cell<bool>,
    pub palette_open: bool,
    pub norms_verified: usize,
    pub norms_pending: usize,
    /// De dónde salió el pack normativo (`embebido` o `archivo: …`).
    pub pack_source: String,
    /// Estado del almacenamiento, mostrado en la barra inferior.
    pub storage_status: String,
    /// Momento en que se cargó la aplicación (para la barra de estado).
    pub loaded_at: Timestamp,
}

impl AppState {
    pub fn new(rows: Vec<SampleRow>) -> Self {
        let count = rows.len();
        let mut table = TableState::new();
        if count > 0 {
            table.set_sort("code", rune_kit::SortDir::Ascending);
        }
        Self {
            nav: Nav::Dashboard,
            rows,
            table,
            dark_override: None,
            system_dark: Cell::new(false),
            palette_open: false,
            norms_verified: 0,
            norms_pending: 0,
            pack_source: "embebido".to_owned(),
            storage_status: "almacenamiento local".to_owned(),
            loaded_at: Timestamp::from_millis(0),
        }
    }

    pub fn is_dark(&self, system_dark: bool) -> bool {
        self.dark_override.unwrap_or(system_dark)
    }

    /// Guarda el tema del sistema para poder invertirlo desde el botón.
    pub fn sync_system_theme(&self, system_dark: bool) {
        self.system_dark.set(system_dark);
    }

    pub fn toggle_theme(&mut self) {
        let current = self.is_dark(self.system_dark.get());
        self.dark_override = Some(!current);
    }

    pub fn select(&mut self, nav: Nav) {
        self.nav = nav;
        self.palette_open = false;
    }

    pub fn toggle_palette(&mut self) {
        self.palette_open = !self.palette_open;
    }

    pub fn close_palette(&mut self) {
        self.palette_open = false;
    }

    pub fn total_rows(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Texto de la barra de estado.
    ///
    /// Es una función del estado y no de la vista, para poder probar que **dice lo que
    /// debe** sin depender de cómo se dibuja.
    pub fn status_line(&self) -> String {
        format!(
            "{} muestras · normas: {} verificadas, {} pendientes ({}) · {} · {}",
            self.total_rows(),
            self.norms_verified,
            self.norms_pending,
            self.pack_source,
            self.storage_status,
            self.nav.label()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(code: &str) -> SampleRow {
        SampleRow {
            code: code.into(),
            material: "Hormigón".into(),
            client: "Andina".into(),
            received_on: "2026-11-18".into(),
            state: "Recibida".into(),
        }
    }

    #[test]
    fn la_navegacion_tiene_seis_secciones_estables() {
        assert_eq!(Nav::ALL.len(), 6);
        for (index, nav) in Nav::ALL.iter().enumerate() {
            assert_eq!(nav.index(), index);
            assert_eq!(Nav::from_index(index), Some(*nav));
        }
        assert_eq!(Nav::from_index(99), None);
    }

    #[test]
    fn seleccionar_una_seccion_cierra_la_paleta() {
        let mut state = AppState::new(vec![]);
        state.toggle_palette();
        assert!(state.palette_open);
        state.select(Nav::Equipment);
        assert_eq!(state.nav, Nav::Equipment);
        assert!(!state.palette_open);
    }

    #[test]
    fn el_tema_sigue_al_sistema_salvo_que_se_fuerce() {
        let mut state = AppState::new(vec![]);
        state.sync_system_theme(true);
        assert!(state.is_dark(true), "sin preferencia, sigue al sistema");
        state.toggle_theme();
        assert_eq!(state.dark_override, Some(false), "invierte el del sistema");
        state.toggle_theme();
        assert_eq!(state.dark_override, Some(true));
    }

    #[test]
    fn el_estado_refleja_las_filas_cargadas() {
        let state = AppState::new(vec![row("H-1"), row("H-2")]);
        assert_eq!(state.total_rows(), 2);
        assert!(!state.is_empty());
        assert!(AppState::new(vec![]).is_empty());
    }

    #[test]
    fn la_barra_de_estado_dice_lo_que_hay() {
        let mut state = AppState::new(vec![row("H-1")]);
        state.norms_verified = 2;
        state.norms_pending = 5;
        state.pack_source = "archivo: propio.json".into();
        state.storage_status = "base local: C:/datos/mamut.db".into();

        let line = state.status_line();
        assert!(line.contains("1 muestras"), "{line}");
        assert!(line.contains("2 verificadas"), "{line}");
        assert!(line.contains("5 pendientes"), "{line}");
        assert!(line.contains("archivo: propio.json"), "{line}");
        assert!(line.contains("mamut.db"), "{line}");
    }

    #[test]
    fn una_fila_se_formatea_a_partir_de_la_muestra() {
        use mamut_core::{Actor, CivilDate};
        use mamut_domain::{Material, Sample, SampleCode};

        let sample = Sample::new(
            SampleCode::new("H-2026-0001").unwrap(),
            Material::Concrete,
            "Bloque 3",
            "Constructora Andina",
            "Torre Norte",
            CivilDate::new(2026, 11, 18).unwrap(),
            Actor::new("u-1", "Ana Torres").unwrap(),
            Timestamp::from_millis(0),
        )
        .unwrap();

        let row = SampleRow::from_sample(&sample);
        assert_eq!(row.code, "H-2026-0001");
        assert_eq!(row.material, "Hormigón");
        assert_eq!(row.received_on, "2026-11-18");
        assert_eq!(row.state, "Recibida");
    }
}
