//! Estado de la interfaz.
//!
//! Es **solo** lo que la pantalla necesita para dibujarse y para atender sus acciones.
//! Las reglas de negocio viven en `mamut-domain`; el acceso a datos, en [`Store`].
//!
//! La vista no modifica este estado: lo hacen los manejadores (`.on_press`, `.on_edit`),
//! que es lo que mantiene la interfaz predecible y probable.

use std::cell::Cell;

use mamut_calculations::{
    CompactionPoint, CylinderCompressionInput, ENGINE_VERSION, PenetrationPoint, RoundingPolicy,
    SievePass, calculate, cbr_from_curve, curve_peak, dry_density, field_density,
    liquid_limit_multi_point, plastic_limit, plasticity_index, sieve_analysis, wet_density,
};
use mamut_core::{Actor, CivilDate, Clock, Force, Length, Mass, SystemClock, Timestamp};
use mamut_domain::{Material, Measurement, ResultRef, Sample, SampleCode, Specimen, TestRecord};
use mamut_reports::{
    GranulometryReport, LaboratoryInfo, ReportData, ReportLine, render_granulometry_pdf,
    render_pdf, verdict_text,
};
use mamut_standards::{Conformity, NormBody, NormRef, NormStatus, TestDescriptor};
use mamut_validation::{PlausibilityLimits, Severity, validate_cylinder_input};
use rune::TextEdit;
use rune_kit::TableState;

use crate::store::Store;

/// Cuántas muestras se cargan en la tabla.
pub const SAMPLE_LIMIT: usize = 500;

/// Identificador del operador local mientras no exista gestión de usuarios.
///
/// El nombre sí lo aporta el formulario: la auditoría registra quién recepcionó la
/// muestra, no solo que «alguien» lo hizo.
pub const LOCAL_ACTOR_ID: &str = "local";

/// Tamaño legible de un archivo, sin truncar a cero.
fn human_size(bytes: i64) -> String {
    let bytes = bytes.max(0);
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Piezas del ajuste del límite líquido: valor, pendiente, R², número de determinaciones,
/// aviso de golpes fuera de rango y las determinaciones originales.
type LimitFitParts = (f64, f64, f64, usize, bool, Vec<(u32, f64)>);

/// Pesadas de un punto de compactación: molde, molde+muestra, volumen, húmeda, seca y los
/// dos derivados (humedad y densidad seca).
type CompactionPointParts = (f64, f64, f64, f64, f64, f64, f64);

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

/// Borrador del formulario de alta de muestra.
///
/// Los campos guardan **texto**, no valores ya convertidos: mientras el usuario escribe,
/// «2026-1» no es una fecha y no debe corregirse a mitad de tecleo. La conversión ocurre
/// al guardar, y su error se muestra en el propio formulario.
pub struct SampleForm {
    pub code: TextEdit,
    pub origin: TextEdit,
    pub client: TextEdit,
    pub project: TextEdit,
    pub received_on: TextEdit,
    pub responsible: TextEdit,
    pub material: Material,
    pub error: Option<String>,
}

impl SampleForm {
    pub fn empty() -> Self {
        Self {
            code: TextEdit::new(""),
            origin: TextEdit::new(""),
            client: TextEdit::new(""),
            project: TextEdit::new(""),
            received_on: TextEdit::new(""),
            responsible: TextEdit::new(""),
            material: Material::Concrete,
            error: None,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::empty();
    }

    /// Fija la fecha de recepción al día de hoy del reloj dado.
    pub fn today(&mut self, clock: &dyn Clock) {
        self.received_on = TextEdit::new(CivilDate::today(clock).to_iso());
    }
}

impl Default for SampleForm {
    fn default() -> Self {
        Self::empty()
    }
}

/// Resultado ya calculado, listo para mostrar.
#[derive(Clone, Debug, PartialEq)]
pub struct OutcomeView {
    pub area_mm2: f64,
    pub stress_mpa: f64,
    pub stress_display: f64,
    pub slenderness: f64,
    pub norm_citation: String,
    /// Veredicto en palabras. Se calcula con el paquete normativo, no aquí.
    pub conformity: String,
}

/// Una fila del formulario de granulometría.
pub struct SieveRow {
    pub size: TextEdit,
    pub retained: TextEdit,
}

/// Resultado de un análisis granulométrico, listo para mostrar.
#[derive(Clone, Debug, PartialEq)]
pub struct SoilsOutcome {
    /// Muestra para la que se calculó: un informe con otra muestra es un error.
    pub sample: String,
    pub total_g: f64,
    pub sieves: Vec<SievePass>,
    /// Cuántos tamices sostienen la curva.
    pub sieve_count: usize,
    /// Algún diámetro cayó en un extremo de la curva: el dato existe pero es un borde.
    pub d_at_boundary: bool,
    pub passing: Vec<(f64, f64)>,
    pub d10: Option<f64>,
    pub d30: Option<f64>,
    pub d60: Option<f64>,
    pub cu: Option<f64>,
    pub cc: Option<f64>,
}

/// Formulario de granulometría.
pub struct SoilsForm {
    /// Código de la muestra a la que se asocia el informe.
    pub sample: TextEdit,
    pub total: TextEdit,
    pub rows: Vec<SieveRow>,
    pub error: Option<String>,
    pub outcome: Option<SoilsOutcome>,
    pub report_path: Option<String>,
    /// Firma de los campos con los que se calculó.
    pub computed_from: Option<String>,
}

impl SoilsForm {
    pub fn empty() -> Self {
        let mut form = Self {
            sample: TextEdit::new(""),
            total: TextEdit::new(""),
            rows: Vec::new(),
            error: None,
            outcome: None,
            report_path: None,
            computed_from: None,
        };
        // Serie habitual de tamices, con el retenido vacío.
        for size in ["9.5", "4.75", "2.0", "0.425", "0.075"] {
            form.rows.push(SieveRow {
                size: TextEdit::new(size),
                retained: TextEdit::new(""),
            });
        }
        form
    }
}

impl Default for SoilsForm {
    fn default() -> Self {
        Self::empty()
    }
}

/// Una determinación del límite líquido en el formulario.
pub struct LimitRow {
    pub blows: TextEdit,
    pub moisture: TextEdit,
}

/// Resultado de límites de Atterberg.
#[derive(Clone, Debug, PartialEq)]
pub struct LimitsOutcome {
    /// Muestra para la que se calculó.
    pub sample: String,
    /// Determinaciones originales: golpes y humedad.
    pub points: Vec<(u32, f64)>,
    pub liquid_limit: f64,
    pub slope: f64,
    pub r_squared: f64,
    pub determinations: usize,
    pub blows_outside_range: bool,
    pub plastic_limit: Option<f64>,
    pub plasticity_index: Option<f64>,
}

/// Formulario de límites de Atterberg.
pub struct LimitsForm {
    pub rows: Vec<LimitRow>,
    pub water: TextEdit,
    pub dry: TextEdit,
    pub tare: TextEdit,
    pub error: Option<String>,
    pub outcome: Option<LimitsOutcome>,
    /// Firma de los campos con los que se calculó.
    pub computed_from: Option<String>,
}

impl LimitsForm {
    pub fn empty() -> Self {
        let mut form = Self {
            rows: Vec::new(),
            water: TextEdit::new(""),
            dry: TextEdit::new(""),
            tare: TextEdit::new(""),
            error: None,
            outcome: None,
            computed_from: None,
        };
        // Tres determinaciones: el mínimo que exige el ajuste.
        for _ in 0..3 {
            form.rows.push(LimitRow {
                blows: TextEdit::new(""),
                moisture: TextEdit::new(""),
            });
        }
        form
    }
}

impl Default for LimitsForm {
    fn default() -> Self {
        Self::empty()
    }
}

/// Resultado de una determinación de densidad en sitio.
#[derive(Clone, Debug, PartialEq)]
pub struct DensityOutcome {
    /// Muestra para la que se calculó.
    pub sample: String,
    pub dry_density: f64,
    pub relative_compaction: f64,
    pub moisture_deviation: f64,
    pub warnings: Vec<&'static str>,
}

/// Formulario de densidad en sitio.
pub struct DensityForm {
    /// Método con el que se midió la densidad en sitio: el cálculo no puede confundirlos.
    pub method: TextEdit,
    pub wet: TextEdit,
    pub moisture: TextEdit,
    pub max_dry: TextEdit,
    pub optimum: TextEdit,
    pub error: Option<String>,
    pub outcome: Option<DensityOutcome>,
    /// Firma de los campos con los que se calculó.
    pub computed_from: Option<String>,
}

impl DensityForm {
    pub fn empty() -> Self {
        Self {
            method: TextEdit::new("nuclear"),
            wet: TextEdit::new(""),
            moisture: TextEdit::new(""),
            max_dry: TextEdit::new(""),
            optimum: TextEdit::new(""),
            error: None,
            outcome: None,
            computed_from: None,
        }
    }
}

impl Default for DensityForm {
    fn default() -> Self {
        Self::empty()
    }
}

/// Un punto de la curva de compactación ya calculado.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompactionPointView {
    /// Pesadas originales del punto. Sin ellas la curva no se puede auditar.
    pub mold_g: f64,
    pub mold_plus_g: f64,
    pub volume_cm3: f64,
    pub wet_sample_g: f64,
    pub dry_sample_g: f64,
    pub moisture: f64,
    pub dry_density: f64,
}

/// Máximo de la curva de compactación, listo para mostrar.
#[derive(Clone, Debug, PartialEq)]
pub struct PeakOutcome {
    /// Muestra para la que se calculó.
    pub sample: String,
    pub optimum_moisture: f64,
    pub max_dry_density: f64,
    pub points_used: usize,
    /// El ajuste quedó por debajo del mayor valor medido: revisar antes de usarlo.
    pub below_max_measured: bool,
}

/// Formulario de compactación.
pub struct CompactionForm {
    /// Firma de las pesadas con las que se calculó el máximo.
    pub computed_from: Option<String>,
    pub mold_plus: TextEdit,
    pub mold: TextEdit,
    pub volume: TextEdit,
    pub wet_sample: TextEdit,
    pub dry_sample: TextEdit,
    pub points: Vec<CompactionPointView>,
    pub error: Option<String>,
    pub peak: Option<PeakOutcome>,
}

impl CompactionForm {
    pub fn empty() -> Self {
        Self {
            computed_from: None,
            mold_plus: TextEdit::new(""),
            mold: TextEdit::new(""),
            volume: TextEdit::new(""),
            wet_sample: TextEdit::new(""),
            dry_sample: TextEdit::new(""),
            points: Vec::new(),
            error: None,
            peak: None,
        }
    }
}

impl Default for CompactionForm {
    fn default() -> Self {
        Self::empty()
    }
}

/// Una fila de la curva carga–penetración.
pub struct CbrRow {
    pub penetration: TextEdit,
    pub load: TextEdit,
}

/// Resultado del CBR.
#[derive(Clone, Debug, PartialEq)]
pub struct CbrOutcome {
    /// Muestra para la que se calculó.
    pub sample: String,
    /// Curva original: penetración y carga unitaria.
    pub points: Vec<(f64, f64)>,
    pub percent: f64,
    pub governing_penetration: f64,
    pub offset: f64,
}

/// Formulario de CBR.
pub struct CbrForm {
    pub rows: Vec<CbrRow>,
    pub error: Option<String>,
    pub outcome: Option<CbrOutcome>,
    /// Firma de los campos con los que se calculó.
    pub computed_from: Option<String>,
}

impl CbrForm {
    pub fn empty() -> Self {
        let mut form = Self {
            rows: Vec::new(),
            error: None,
            outcome: None,
            computed_from: None,
        };
        // Penetraciones habituales del ensayo: la curva debe pasar de 5,08 mm.
        for penetration in ["0", "0.5", "1.0", "2.54", "5.08"] {
            form.rows.push(CbrRow {
                penetration: TextEdit::new(penetration),
                load: TextEdit::new(""),
            });
        }
        form
    }
}

impl Default for CbrForm {
    fn default() -> Self {
        Self::empty()
    }
}

/// Borrador del ensayo de compresión de cilindros.
pub struct TestForm {
    pub sample_code: TextEdit,
    pub designation: TextEdit,
    pub diameter: TextEdit,
    pub height: TextEdit,
    pub mass: TextEdit,
    pub age: TextEdit,
    pub load: TextEdit,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub outcome: Option<OutcomeView>,
    /// Entrada exacta que produjo el resultado: se guarda con él como huella.
    input: Option<CylinderCompressionInput>,
    /// Identificador del ensayo ya guardado.
    pub saved: Option<String>,
    /// Firma de los campos con los que se calculó: si cambian, el resultado caduca.
    pub computed_from: Option<String>,
    pub report_path: Option<String>,
}

impl TestForm {
    pub fn empty() -> Self {
        Self {
            sample_code: TextEdit::new(""),
            designation: TextEdit::new("C1"),
            diameter: TextEdit::new("150"),
            height: TextEdit::new("300"),
            mass: TextEdit::new(""),
            age: TextEdit::new("28"),
            load: TextEdit::new(""),
            error: None,
            warnings: Vec::new(),
            outcome: None,
            input: None,
            saved: None,
            report_path: None,
            computed_from: None,
        }
    }

    /// Limpia el resultado, pero **no** los datos: corregir una carga no debe obligar a
    /// reescribir las dimensiones.
    pub fn clear_result(&mut self) {
        self.outcome = None;
        self.input = None;
        self.computed_from = None;
        self.saved = None;
        self.report_path = None;
        self.error = None;
        self.warnings.clear();
    }
}

impl Default for TestForm {
    fn default() -> Self {
        Self::empty()
    }
}

/// Una fila del historial de ensayos de una muestra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestRow {
    pub performed_on: String,
    pub designation: String,
    pub stress: String,
    pub state: String,
}

/// Acepta coma decimal (`150,5`), que es como se teclea en español.
fn parse_number(text: &str, field: &'static str) -> std::result::Result<f64, String> {
    let normalized = text.trim().replace(',', ".");
    if normalized.is_empty() {
        return Err(format!("falta «{field}»"));
    }
    normalized
        .parse::<f64>()
        .map_err(|_| format!("«{field}» no es un número: «{}»", text.trim()))
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
    /// Momento en que se cargó la aplicación.
    pub loaded_at: Timestamp,
    /// Total de muestras en la base (la tabla muestra solo las últimas).
    pub samples_total: usize,
    /// Total de ensayos registrados.
    pub tests_total: usize,
    /// Almacenamiento. `None` en pruebas de vista que no necesitan base.
    pub store: Option<Store>,
    pub form: SampleForm,
    pub form_open: bool,
    /// Formulario del ensayo de compresión.
    pub test_form: TestForm,
    /// Formulario de granulometría.
    pub soils: SoilsForm,
    /// Formulario de límites de Atterberg.
    pub limits: LimitsForm,
    /// Formulario de densidad en sitio.
    pub density: DensityForm,
    /// Formulario de compactación.
    pub compaction: CompactionForm,
    /// Formulario de CBR.
    pub cbr: CbrForm,
    /// Historial del ensayo para la muestra escrita en el formulario.
    pub test_history: Vec<TestRow>,
    /// Descriptor del ensayo, tomado del paquete normativo al arrancar.
    pub descriptor: Option<TestDescriptor>,
    /// Código de muestra sobre el que actúan las herramientas (adjuntos, anulación).
    pub tool_code: TextEdit,
    /// Ruta del archivo a adjuntar.
    pub tool_path: TextEdit,
    /// Motivo de la anulación.
    pub tool_reason: TextEdit,
    /// Adjuntos de la muestra seleccionada en las herramientas.
    pub attachments: Vec<String>,
    /// Serie resistencia–edad de esa muestra.
    pub series: Vec<(u32, f64)>,
    /// Muestra cuya anulación está a la espera de confirmación.
    pub pending_void: Option<String>,
    /// Código de la muestra cuyos informes se consultan.
    pub reports_code: TextEdit,
    /// Informes emitidos de esa muestra, ya formateados.
    pub reports_list: Vec<String>,
    /// Error al consultar los informes.
    pub reports_error: Option<String>,
    /// Mensaje efímero tras una acción correcta.
    pub flash: Option<String>,
}

impl AppState {
    pub fn new(rows: Vec<SampleRow>) -> Self {
        let samples_total = rows.len();
        let mut table = TableState::new();
        table.set_sort("code", rune_kit::SortDir::Ascending);
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
            samples_total,
            tests_total: 0,
            store: None,
            form: SampleForm::empty(),
            form_open: false,
            test_form: TestForm::empty(),
            soils: SoilsForm::empty(),
            limits: LimitsForm::empty(),
            density: DensityForm::empty(),
            compaction: CompactionForm::empty(),
            cbr: CbrForm::empty(),
            test_history: Vec::new(),
            descriptor: None,
            tool_code: TextEdit::new(""),
            tool_path: TextEdit::new(""),
            tool_reason: TextEdit::new(""),
            attachments: Vec::new(),
            series: Vec::new(),
            pending_void: None,
            reports_code: TextEdit::new(String::new()),
            reports_list: Vec::new(),
            reports_error: None,
            flash: None,
        }
    }

    /// Estado con almacenamiento: carga las filas y los contadores reales.
    pub fn with_store(store: Store) -> Self {
        let mut state = Self::new(Vec::new());
        state.store = Some(store);
        state.reload();
        state
    }

    /// Relee lo que la tabla y los indicadores necesitan.
    pub fn reload(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        match store.sample_rows(SAMPLE_LIMIT) {
            Ok(rows) => self.rows = rows,
            Err(error) => self.flash = Some(format!("no se pudieron leer las muestras: {error}")),
        }
        match store.sample_count() {
            Ok(total) => self.samples_total = total,
            Err(error) => self.flash = Some(format!("no se pudo contar las muestras: {error}")),
        }
        match store.test_count() {
            Ok(total) => self.tests_total = total,
            Err(error) => self.flash = Some(format!("no se pudieron contar los ensayos: {error}")),
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

    /// Abre el formulario de alta con la fecha de hoy ya puesta.
    pub fn open_form(&mut self) {
        self.form.reset();
        self.form.today(&SystemClock);
        self.form_open = true;
        self.flash = None;
    }

    pub fn cancel_form(&mut self) {
        self.form.reset();
        self.form_open = false;
    }

    pub fn set_material(&mut self, material: Material) {
        self.form.material = material;
    }

    /// Valida el formulario y, si es correcto, guarda la muestra.
    ///
    /// No hay ninguna ruta que guarde un borrador inválido: si algo falla, el error se
    /// muestra en el formulario y no se escribe nada.
    pub fn save_form(&mut self) {
        self.form.error = None;
        self.flash = None;

        let code = self.form.code.text.trim().to_owned();
        let origin = self.form.origin.text.trim().to_owned();
        let client = self.form.client.text.trim().to_owned();
        let project = self.form.project.text.trim().to_owned();
        let date_text = self.form.received_on.text.trim().to_owned();
        let responsible = self.form.responsible.text.trim().to_owned();
        let material = self.form.material;

        let build = || -> std::result::Result<Sample, String> {
            let code = SampleCode::new(code).map_err(|error| error.to_string())?;
            let received_on = CivilDate::parse(&date_text).map_err(|error| error.to_string())?;
            let actor =
                Actor::new(LOCAL_ACTOR_ID, responsible).map_err(|error| error.to_string())?;
            // `Sample::new` valida procedencia, cliente y proyecto no vacíos.
            Sample::new(
                code,
                material,
                origin,
                client,
                project,
                received_on,
                actor,
                SystemClock.now(),
            )
            .map_err(|error| error.to_string())
        };

        let sample = match build() {
            Ok(sample) => sample,
            Err(message) => {
                self.form.error = Some(message);
                return;
            }
        };

        let Some(store) = &self.store else {
            self.form.error = Some("no hay almacenamiento disponible".to_owned());
            return;
        };

        match store.create_sample(&sample, &sample.responsible, SystemClock.now()) {
            Ok(()) => {
                self.flash = Some(format!("Muestra {} registrada", sample.code));
                self.form.reset();
                self.form_open = false;
                self.reload();
            }
            Err(message) => self.form.error = Some(message),
        }
    }

    /// Norma del ensayo, tomada del descriptor del paquete normativo.
    ///
    /// Si no hay descriptor (pruebas de vista), se usa una referencia pendiente: el
    /// cálculo funciona igual y la conformidad queda sin evaluar, que es lo honesto.
    fn current_norm(&self) -> NormRef {
        if let Some(norm) = self.descriptor.as_ref().and_then(|d| d.norm.clone()) {
            return norm;
        }
        match NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2026,
            "Compresión de cilindros",
            NormStatus::PendingReview,
        ) {
            Ok(norm) => norm,
            // Valores literales: la construcción no puede fallar.
            Err(_) => unreachable!("la referencia por defecto es válida"),
        }
    }

    pub fn set_descriptor(&mut self, descriptor: TestDescriptor) {
        self.descriptor = Some(descriptor);
    }

    /// Valida y calcula el ensayo con lo que hay en el formulario.
    pub fn calculate_test(&mut self) {
        self.test_form.error = None;
        self.test_form.warnings.clear();
        self.test_form.outcome = None;
        self.test_form.input = None;
        self.test_form.saved = None;
        self.test_form.report_path = None;

        let parsed = (|| -> std::result::Result<CylinderCompressionInput, String> {
            let diameter = parse_number(&self.test_form.diameter.text, "diámetro")?;
            let height = parse_number(&self.test_form.height.text, "altura")?;
            let mass = parse_number(&self.test_form.mass.text, "masa")?;
            let age_value = parse_number(&self.test_form.age.text, "edad")?;
            let load = parse_number(&self.test_form.load.text, "carga máxima")?;

            if age_value < 0.0 || age_value.fract() != 0.0 {
                return Err("la edad debe ser un número entero de días".to_owned());
            }
            // Un `as u32` saturaría en silencio: 1e20 días se convertiría en u32::MAX.
            if age_value > 3650.0 {
                return Err("la edad no puede superar los 3650 días: revisa el dato".to_owned());
            }

            Ok(CylinderCompressionInput {
                diameter: Length::positive_millimetres(diameter, "diámetro")
                    .map_err(|error| error.to_string())?,
                height: Length::positive_millimetres(height, "altura")
                    .map_err(|error| error.to_string())?,
                mass: Mass::positive_grams(mass, "masa").map_err(|error| error.to_string())?,
                max_load: Force::positive_kilonewtons(load, "carga máxima")
                    .map_err(|error| error.to_string())?,
                age_days: age_value as u32,
                norm: self.current_norm(),
                rounding: RoundingPolicy::PROVISIONAL,
            })
        })();

        let input = match parsed {
            Ok(input) => input,
            Err(message) => {
                self.test_form.error = Some(message);
                return;
            }
        };

        let validation = validate_cylinder_input(&input, &PlausibilityLimits::default());
        self.test_form.warnings = validation
            .findings
            .iter()
            .filter(|finding| finding.severity == Severity::Warning)
            .map(|finding| finding.message.clone())
            .collect();

        if validation.has_errors() {
            let message = validation
                .errors()
                .map(|finding| finding.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            self.test_form.error = Some(message);
            return;
        }

        match calculate(&input) {
            Ok(outcome) => {
                let conformity = Conformity::evaluate(
                    outcome.stress.megapascals_value(),
                    &input.norm,
                    None,
                    None,
                );
                self.test_form.outcome = Some(OutcomeView {
                    area_mm2: outcome.area.square_millimetres_value(),
                    stress_mpa: outcome.stress.megapascals_value(),
                    stress_display: outcome.stress_display,
                    slenderness: outcome.slenderness,
                    norm_citation: outcome.norm_citation,
                    conformity: verdict_text(&conformity),
                });
                self.test_form.input = Some(input);
                self.test_form.computed_from = Some(self.test_signature());
            }
            Err(error) => self.test_form.error = Some(error.to_string()),
        }
    }

    /// Guarda el ensayo ya calculado: espécimen, ensayo, mediciones y resultado.
    pub fn save_test(&mut self) {
        self.test_form.error = None;

        let Some(input) = self.test_form.input.clone() else {
            self.test_form.error = Some("calcula el ensayo antes de guardarlo".to_owned());
            return;
        };
        if let Err(message) =
            self.require_fresh(&self.test_signature(), &self.test_form.computed_from)
        {
            self.test_form.error = Some(message);
            return;
        }

        let Some(store) = &self.store else {
            self.test_form.error = Some("no hay almacenamiento disponible".to_owned());
            return;
        };

        let code = self.test_form.sample_code.text.trim().to_owned();
        let sample = match store.find_sample(&code) {
            Ok(Some(sample)) => sample,
            Ok(None) => {
                self.test_form.error = Some(format!(
                    "no existe la muestra «{code}»: regístrala antes de ensayarla"
                ));
                return;
            }
            Err(error) => {
                self.test_form.error = Some(error);
                return;
            }
        };

        if !sample.accepts_specimens() {
            self.test_form.error = Some(format!(
                "la muestra «{code}» está anulada o completada y no acepta ensayos"
            ));
            return;
        }

        let designation = self.test_form.designation.text.trim().to_owned();
        let now = SystemClock.now();

        let build = (|| -> std::result::Result<(Specimen, TestRecord, Vec<Measurement>, ResultRef), String> {
            let specimen = Specimen::new(
                sample.id,
                designation,
                input.diameter,
                input.height,
                input.mass,
                input.age_days,
                now,
            )
            .map_err(|error| error.to_string())?;

            let descriptor = self
                .descriptor
                .clone()
                .ok_or_else(|| "no hay descriptor de ensayo cargado".to_owned())?;

            let test = TestRecord::new(
                descriptor,
                specimen.id,
                CivilDate::today(&SystemClock),
                sample.responsible.clone(),
                now,
            );

            // Los datos originales se guardan tal como se teclearon, salvo la coma decimal:
            // el dominio exige un número parseable, y en español se escribe con coma.
            let canonical = |text: &str| text.trim().replace(',', ".");
            let measurements = vec![
                Measurement::new("diametro", canonical(&self.test_form.diameter.text), "mm", now)
                    .map_err(|error| error.to_string())?,
                Measurement::new("altura", canonical(&self.test_form.height.text), "mm", now)
                    .map_err(|error| error.to_string())?,
                Measurement::new("masa", canonical(&self.test_form.mass.text), "g", now)
                    .map_err(|error| error.to_string())?,
                Measurement::new("carga_maxima", canonical(&self.test_form.load.text), "kN", now)
                    .map_err(|error| error.to_string())?,
            ];

            let outcome = calculate(&input).map_err(|error| error.to_string())?;
            let fingerprint = input.fingerprint().map_err(|error| error.to_string())?;
            let result = ResultRef::new(
                outcome.stress.megapascals_value(),
                "MPa",
                ENGINE_VERSION,
                fingerprint,
                now,
            )
            .map_err(|error| error.to_string())?;

            Ok((specimen, test, measurements, result))
        })();

        let (specimen, test, measurements, result) = match build {
            Ok(bundle) => bundle,
            Err(message) => {
                self.test_form.error = Some(message);
                return;
            }
        };

        let test_id = test.id.to_string();
        match store.save_test_bundle(
            &specimen,
            &test,
            &measurements,
            &result,
            &sample.responsible,
            now,
        ) {
            Ok(()) => {
                // Primer ensayo de la muestra: pasa a «en ensayo». Es el último uso del
                // préstamo del almacén antes de tocar el estado.
                let _ = store.begin_testing(&code, &sample.responsible, now);
                self.test_form.saved = Some(test_id);
                self.flash = Some(format!("Ensayo guardado para la muestra {code}"));
                self.load_test_history();
                // Las herramientas siguen a la muestra ensayada. El código se rellena para
                // la lectura (adjuntos y gráfico); anular exige confirmación aparte.
                self.tool_code = TextEdit::new(code);
                self.load_tools();
                self.reload();
            }
            Err(message) => self.test_form.error = Some(message),
        }
    }

    /// Emite el informe PDF del ensayo guardado.
    pub fn generate_report(&mut self) {
        self.test_form.error = None;
        self.test_form.report_path = None;

        let Some(input) = self.test_form.input.clone() else {
            self.test_form.error = Some("no hay resultado calculado".to_owned());
            return;
        };
        if let Err(message) =
            self.require_fresh(&self.test_signature(), &self.test_form.computed_from)
        {
            self.test_form.error = Some(message);
            return;
        }

        if self.test_form.saved.is_none() {
            self.test_form.error = Some("guarda el ensayo antes de emitir el informe".to_owned());
            return;
        }
        let Some(store) = &self.store else {
            self.test_form.error = Some("no hay almacenamiento disponible".to_owned());
            return;
        };

        let code = self.test_form.sample_code.text.trim().to_owned();
        let sample = match store.find_sample(&code) {
            Ok(Some(sample)) => sample,
            Ok(None) => {
                self.test_form.error = Some(format!("no existe la muestra «{code}»"));
                return;
            }
            Err(error) => {
                self.test_form.error = Some(error);
                return;
            }
        };

        let outcome = match calculate(&input) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.test_form.error = Some(error.to_string());
                return;
            }
        };
        let conformity =
            Conformity::evaluate(outcome.stress.megapascals_value(), &input.norm, None, None);
        let today = CivilDate::today(&SystemClock);

        let Some(laboratory) = LaboratoryInfo::new("MAMUT Lab") else {
            self.test_form.error = Some("identidad del laboratorio no configurada".to_owned());
            return;
        };

        let data = ReportData {
            laboratory,
            sample_code: code.clone(),
            client: sample.client.clone(),
            project: sample.project.clone(),
            received_on: sample.received_on,
            test_name: self
                .descriptor
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "Compresión de cilindros".to_owned()),
            method_label: self
                .descriptor
                .as_ref()
                .map(|d| d.method_kind.label().to_owned())
                .unwrap_or_else(|| "Normativo".to_owned()),
            norm_citation: Some(outcome.norm_citation.clone()),
            performed_on: today,
            operator: sample.responsible.name.clone(),
            equipment: None,
            measurements: vec![
                ReportLine::with_unit("Designación", self.test_form.designation.text.trim(), ""),
                ReportLine::with_unit("Edad", self.test_form.age.text.trim(), "días"),
                ReportLine::with_unit("Diámetro", self.test_form.diameter.text.trim(), "mm"),
                ReportLine::with_unit("Altura", self.test_form.height.text.trim(), "mm"),
                ReportLine::with_unit("Masa", self.test_form.mass.text.trim(), "g"),
                ReportLine::with_unit("Carga máxima", self.test_form.load.text.trim(), "kN"),
            ],
            calculations: vec![
                ReportLine::with_unit(
                    "Área de sección",
                    format!("{:.2}", outcome.area.square_millimetres_value()),
                    "mm²",
                ),
                ReportLine::with_unit(
                    "Relación altura/diámetro",
                    format!("{:.2}", outcome.slenderness),
                    "",
                ),
            ],
            result: ReportLine::with_unit(
                "Resistencia a compresión",
                format!("{:.2}", outcome.stress_display),
                "MPa",
            ),
            conformity,
            observations: String::new(),
            issued_on: today,
            responsible: sample.responsible.name.clone(),
        };

        let bytes = render_pdf(&data);
        match store.save_pdf(&format!("informe-{code}"), &bytes) {
            Ok(path) => {
                let shown = path.display().to_string();
                let test_id = self.test_form.saved.clone().unwrap_or_default();

                // El informe deja constancia: registro con hash y ensayo en estado informado.
                let registration = (|| -> crate::store::StoreResult<()> {
                    let actor = Actor::new(LOCAL_ACTOR_ID, "Operador de laboratorio")
                        .map_err(|error| error.to_string())?;
                    let at = SystemClock.now();
                    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
                    store.save_report(&code, "compresion", &test_id, &path, &bytes, &actor, at)?;
                    store.mark_test_reported(&test_id, &shown, &actor, at)?;
                    Ok(())
                })();

                match registration {
                    Ok(()) => {
                        self.test_form.report_path = Some(shown.clone());
                        self.flash = Some(format!("Informe generado: {shown}"));
                    }
                    Err(message) => {
                        self.test_form.error = Some(format!(
                            "informe emitido en {shown}, pero no quedó registrado: {message}"
                        ));
                    }
                }
            }
            Err(message) => self.test_form.error = Some(message),
        }
    }

    /// Relee el historial de ensayos de la muestra que hay en el formulario.
    pub fn load_test_history(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        let code = self.test_form.sample_code.text.trim().to_owned();
        if code.is_empty() {
            self.test_history.clear();
            return;
        }
        match store.tests_for_sample(&code) {
            Ok(tests) => {
                self.test_history = tests
                    .iter()
                    .map(|test| TestRow {
                        performed_on: test.performed_on.to_iso(),
                        designation: store
                            .specimen_designation(test.specimen_id)
                            .unwrap_or_else(|_| "—".to_owned()),
                        stress: test
                            .result
                            .as_ref()
                            .map(|result| format!("{:.2} {}", result.value, result.unit))
                            .unwrap_or_else(|| "—".to_owned()),
                        state: test.state.label().to_owned(),
                    })
                    .collect();
            }
            Err(error) => {
                self.test_history.clear();
                self.flash = Some(format!("no se pudo leer el historial: {error}"));
            }
        }
    }

    /// Relee adjuntos y serie de la muestra de las herramientas.
    pub fn load_tools(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        let code = self.tool_code.text.trim().to_owned();
        if code.is_empty() {
            self.attachments.clear();
            self.series.clear();
            return;
        }
        match store.attachments_for(&code) {
            Ok(records) => {
                self.attachments = records
                    .iter()
                    .map(|record| {
                        format!(
                            "{} · {} · {}…",
                            record.file_name,
                            human_size(record.bytes),
                            &record.sha256[..12]
                        )
                    })
                    .collect();
            }
            Err(error) => self.flash = Some(format!("no se pudieron leer los adjuntos: {error}")),
        }
        match store.series_for_sample(&code) {
            Ok(series) => self.series = series,
            Err(error) => self.flash = Some(format!("no se pudo leer la serie: {error}")),
        }
    }

    /// Adjunta el archivo indicado a la muestra indicada.
    pub fn attach_tool(&mut self) {
        let code = self.tool_code.text.trim().to_owned();
        let path = self.tool_path.text.trim().to_owned();
        if code.is_empty() || path.is_empty() {
            self.flash = Some("indica la muestra y la ruta del archivo".to_owned());
            return;
        }
        let Some(store) = &self.store else {
            self.flash = Some("no hay almacenamiento disponible".to_owned());
            return;
        };
        match store.attach_file(
            &code,
            std::path::Path::new(&path),
            &Actor::new(LOCAL_ACTOR_ID, "Operador de laboratorio")
                .unwrap_or_else(|_| unreachable!("el actor local es válido")),
            SystemClock.now(),
        ) {
            Ok(summary) => {
                self.flash = Some(format!("Adjuntado: {summary}"));
                self.load_tools();
            }
            Err(message) => self.flash = Some(message),
        }
    }

    /// Anula la muestra indicada, con su motivo.
    pub fn void_tool(&mut self) {
        let code = self.tool_code.text.trim().to_owned();
        let reason = self.tool_reason.text.trim().to_owned();
        if code.is_empty() {
            self.flash = Some("indica la muestra a anular".to_owned());
            return;
        }
        if reason.is_empty() {
            self.flash = Some("la anulación exige un motivo".to_owned());
            return;
        }
        // Anular no se deshace: exige un segundo clic sobre la misma muestra.
        if self.pending_void.as_deref() != Some(code.as_str()) {
            self.pending_void = Some(code.clone());
            self.flash = Some(format!(
                "¿Anular «{code}»? Vuelve a pulsar «Anular» para confirmarlo"
            ));
            return;
        }
        self.pending_void = None;
        let Some(store) = &self.store else {
            self.flash = Some("no hay almacenamiento disponible".to_owned());
            return;
        };
        let actor = match Actor::new(LOCAL_ACTOR_ID, "Operador de laboratorio") {
            Ok(actor) => actor,
            Err(_) => return,
        };
        match store.void_sample(&code, &actor, SystemClock.now(), &reason) {
            Ok(()) => {
                self.flash = Some(format!("Muestra {code} anulada"));
                // El motivo no se reutiliza para otra muestra.
                self.tool_reason = TextEdit::new("");
                self.reload();
                self.load_tools();
            }
            Err(message) => self.flash = Some(message),
        }
    }

    /// Exporta las muestras y deja la ruta en el aviso.
    pub fn export_tool(&mut self, format: crate::store::ExportFormat) {
        let Some(store) = &self.store else {
            self.flash = Some("no hay almacenamiento disponible".to_owned());
            return;
        };
        match store.export_samples(format) {
            Ok((path, count)) => {
                // Si se alcanzó el tope, el archivo está incompleto y hay que decirlo.
                self.flash = Some(if count >= 10_000 {
                    format!(
                        "Exportadas {count} muestras (tope): {} — hay más, exporta por partes",
                        path.display()
                    )
                } else {
                    format!("Exportadas {count} muestras: {}", path.display())
                });
            }
            Err(message) => self.flash = Some(message),
        }
    }

    /// Añade una fila de tamiz al formulario de granulometría.
    pub fn soils_add_row(&mut self) {
        self.soils.rows.push(SieveRow {
            size: TextEdit::new(""),
            retained: TextEdit::new(""),
        });
    }

    /// Calcula la granulometría con lo que hay en el formulario.
    ///
    /// Una fila existe **cuando tiene masa retenida**: el formulario trae los tamices
    /// habituales ya escritos y deja el retenido vacío, así que exigir ambos datos obligaría
    /// a borrar filas antes de poder calcular. Una fila con retenido pero sin abertura sí es
    /// un error: significa que falta el tamiz.
    pub fn soils_calculate(&mut self) {
        self.soils.error = None;
        self.soils.outcome = None;
        // Un informe anterior ya no describe este cálculo.
        self.soils.report_path = None;

        let parsed = (|| -> std::result::Result<(f64, Vec<SievePass>), String> {
            let total = parse_number(&self.soils.total.text, "masa seca total")?;
            let mut sieves = Vec::new();
            for (index, row) in self.soils.rows.iter().enumerate() {
                let size_text = row.size.text.trim();
                let retained_text = row.retained.text.trim();
                if retained_text.is_empty() {
                    continue;
                }
                if size_text.is_empty() {
                    return Err(format!(
                        "la fila {} tiene masa retenida pero no abertura de tamiz",
                        index + 1
                    ));
                }
                sieves.push(SievePass {
                    size_mm: parse_number(size_text, "abertura del tamiz")?,
                    retained_g: parse_number(retained_text, "masa retenida")?,
                });
            }
            if sieves.is_empty() {
                return Err("no hay ningún tamiz con datos".to_owned());
            }
            Ok((total, sieves))
        })();

        let (total, sieves) = match parsed {
            Ok(values) => values,
            Err(message) => {
                self.soils.error = Some(message);
                return;
            }
        };

        match sieve_analysis(total, &sieves) {
            Ok(outcome) => {
                let mut sorted = sieves.clone();
                sorted.sort_by(|a, b| b.size_mm.total_cmp(&a.size_mm));
                self.soils.outcome = Some(SoilsOutcome {
                    sample: self.soils.sample.text.trim().to_owned(),
                    total_g: total,
                    sieves: sorted,
                    sieve_count: outcome.sieve_count,
                    d_at_boundary: outcome.d_at_boundary,
                    passing: outcome.passing,
                    d10: outcome.d10,
                    d30: outcome.d30,
                    d60: outcome.d60,
                    cu: outcome.cu,
                    cc: outcome.cc,
                });
                self.soils.computed_from = Some(self.soils_signature());
                self.soils.report_path = None;
            }
            Err(error) => self.soils.error = Some(error.to_string()),
        }
    }

    /// Emite el informe PDF de granulometría.
    pub fn generate_granulometry_report(&mut self) {
        self.soils.error = None;
        self.soils.report_path = None;

        let Some(outcome) = self.soils.outcome.clone() else {
            self.soils.error = Some("calcula la granulometría antes de emitir el informe".into());
            return;
        };
        if let Err(message) = self.require_fresh(&self.soils_signature(), &self.soils.computed_from)
        {
            self.soils.error = Some(message);
            return;
        }

        let code = self.soils.sample.text.trim().to_owned();
        let sample = match self.require_report_sample(&code, &outcome.sample) {
            Ok(sample) => sample,
            Err(message) => {
                self.soils.error = Some(message);
                return;
            }
        };
        let Some(store) = &self.store else {
            self.soils.error = Some("no hay almacenamiento disponible".into());
            return;
        };
        let Some(laboratory) = LaboratoryInfo::new("MAMUT Lab") else {
            self.soils.error = Some("identidad del laboratorio no configurada".into());
            return;
        };

        let mut sieves = Vec::with_capacity(outcome.sieves.len());
        for sieve in &outcome.sieves {
            let passing = outcome
                .passing
                .iter()
                .find(|(size, _)| (*size - sieve.size_mm).abs() < 1e-9)
                .map(|(_, passing)| *passing)
                .unwrap_or(0.0);
            sieves.push((sieve.size_mm, sieve.retained_g, 100.0 - passing, passing));
        }

        let today = CivilDate::today(&SystemClock);
        let data = GranulometryReport {
            laboratory,
            sample_code: code.clone(),
            client: sample.client.clone(),
            project: sample.project.clone(),
            received_on: sample.received_on,
            performed_on: today,
            operator: sample.responsible.name.clone(),
            total_dry_g: outcome.total_g,
            sieves,
            d10: outcome.d10,
            d30: outcome.d30,
            d60: outcome.d60,
            cu: outcome.cu,
            cc: outcome.cc,
            norm_citation: Some(
                "Procedimiento TexDOT Tex-110-E (abril 2024) — equivalencia ASTM D6913 / \
                 AASHTO T 27 pendiente de verificar"
                    .to_owned(),
            ),
            observations: String::new(),
            issued_on: today,
            responsible: sample.responsible.name.clone(),
        };

        if let Err(message) = data.validate() {
            self.soils.error = Some(message);
            return;
        }

        let bytes = render_granulometry_pdf(&data);
        match store.save_pdf(&format!("granulometria-{code}"), &bytes) {
            Ok(path) => {
                let shown = path.display().to_string();
                let inputs = serde_json::json!({
                    "total_g": outcome.total_g,
                    "tamices": outcome.sieves.iter().map(|sieve| {
                        serde_json::json!({
                            "abertura_mm": sieve.size_mm,
                            "retenido_g": sieve.retained_g,
                        })
                    }).collect::<Vec<_>>(),
                })
                .to_string();
                // El resultado de una granulometría es la curva; se guarda como valor el
                // porcentaje que pasa el tamiz más fino. Un D60 inexistente no se convierte
                // en 0,0 mm, que es un diámetro imposible.
                let headline = outcome
                    .passing
                    .last()
                    .map(|(_, passing)| *passing)
                    .unwrap_or(0.0);
                match self.register_emission(
                    "granulometria",
                    "SOIL-GRANULOMETRIA",
                    &inputs,
                    headline,
                    "% que pasa el tamiz más fino",
                    &shown,
                ) {
                    Ok(()) => {
                        self.soils.report_path = Some(shown.clone());
                        self.flash = Some(format!("Informe de granulometría: {shown}"));
                    }
                    Err(message) => {
                        self.soils.error = Some(format!(
                            "informe emitido en {shown}, pero no quedó registrado: {message}"
                        ));
                    }
                }
            }
            Err(message) => self.soils.error = Some(message),
        }
    }

    /// Añade una determinación al límite líquido.
    pub fn limits_add_row(&mut self) {
        self.limits.rows.push(LimitRow {
            blows: TextEdit::new(""),
            moisture: TextEdit::new(""),
        });
    }

    /// Calcula límite líquido, límite plástico e índice de plasticidad.
    ///
    /// El límite plástico es opcional: sin sus tres masas se informa solo el líquido. Con
    /// las tres, se calcula también el índice.
    pub fn limits_calculate(&mut self) {
        self.limits.error = None;
        self.limits.outcome = None;

        let parsed = (|| -> std::result::Result<LimitFitParts, String> {
            let mut points = Vec::new();
            for (index, row) in self.limits.rows.iter().enumerate() {
                let blows = row.blows.text.trim();
                let moisture = row.moisture.text.trim();
                if blows.is_empty() && moisture.is_empty() {
                    continue;
                }
                if blows.is_empty() || moisture.is_empty() {
                    return Err(format!(
                        "la determinación {} está a medias: indica golpes y humedad",
                        index + 1
                    ));
                }
                let blows_value = parse_number(blows, "golpes")?;
                if blows_value <= 0.0 || blows_value.fract() != 0.0 {
                    return Err(format!(
                        "los golpes deben ser un entero positivo (fila {})",
                        index + 1
                    ));
                }
                points.push(mamut_calculations::LimitPoint {
                    blows: blows_value as u32,
                    moisture_percent: parse_number(moisture, "humedad")?,
                });
            }
            if points.is_empty() {
                return Err("no hay ninguna determinación de límite líquido".to_owned());
            }
            let fit = liquid_limit_multi_point(&points).map_err(|error| error.to_string())?;
            Ok((
                fit.value,
                fit.slope,
                fit.r_squared,
                fit.determinations,
                fit.has_blows_outside_range,
                points
                    .iter()
                    .map(|point| (point.blows, point.moisture_percent))
                    .collect(),
            ))
        })();

        let (liquid_limit, slope, r_squared, determinations, blows_outside_range, points) =
            match parsed {
                Ok(values) => values,
                Err(message) => {
                    self.limits.error = Some(message);
                    return;
                }
            };

        // Límite plástico: opcional, pero si se empieza hay que terminarlo.
        let water = self.limits.water.text.trim();
        let dry = self.limits.dry.text.trim();
        let tare = self.limits.tare.text.trim();
        let filled = [water, dry, tare]
            .iter()
            .filter(|text| !text.is_empty())
            .count();

        let (plastic, index) = if filled == 0 {
            (None, None)
        } else if filled < 3 {
            self.limits.error = Some(
                "para el límite plástico hacen falta las tres masas: agua, seca y tara".into(),
            );
            return;
        } else {
            let plastic = match (|| -> std::result::Result<f64, String> {
                let water = parse_number(water, "masa de agua")?;
                let dry = parse_number(dry, "masa seca")?;
                let tare = parse_number(tare, "tara")?;
                plastic_limit(water, dry, tare).map_err(|error| error.to_string())
            })() {
                Ok(value) => value,
                Err(message) => {
                    self.limits.error = Some(message);
                    return;
                }
            };
            let index = match plasticity_index(liquid_limit, plastic) {
                Ok(value) => value,
                Err(error) => {
                    self.limits.error = Some(error.to_string());
                    return;
                }
            };
            (Some(plastic), Some(index))
        };

        self.limits.computed_from = Some(self.limits_signature());
        self.limits.outcome = Some(LimitsOutcome {
            sample: self.soils.sample.text.trim().to_owned(),
            points,
            liquid_limit,
            slope,
            r_squared,
            determinations,
            blows_outside_range,
            plastic_limit: plastic,
            plasticity_index: index,
        });
    }

    /// Calcula la densidad seca en sitio y el grado de compactación.
    pub fn density_calculate(&mut self) {
        self.density.error = None;
        self.density.outcome = None;

        let parsed = (|| -> std::result::Result<mamut_calculations::FieldDensity, String> {
            let wet = parse_number(&self.density.wet.text, "densidad húmeda")?;
            let moisture = parse_number(&self.density.moisture.text, "humedad")?;
            let max_dry = parse_number(&self.density.max_dry.text, "densidad seca máxima")?;
            let optimum = parse_number(&self.density.optimum.text, "humedad óptima")?;
            field_density(wet, moisture, max_dry, optimum).map_err(|error| error.to_string())
        })();

        match parsed {
            Ok(result) => {
                self.density.computed_from = Some(self.density_signature());
                self.density.outcome = Some(DensityOutcome {
                    sample: self.soils.sample.text.trim().to_owned(),
                    dry_density: result.dry_density_g_cm3,
                    relative_compaction: result.relative_compaction_percent,
                    moisture_deviation: result.moisture_deviation_points,
                    warnings: result.warnings,
                });
            }
            Err(message) => self.density.error = Some(message),
        }
    }

    /// Calcula un punto de la curva de compactación y lo añade a la lista.
    ///
    /// El punto se calcula desde las masas del molde y de la muestra: pedir la densidad seca
    /// ya calculada convertiría la pantalla en un formulario de transcripción.
    pub fn compaction_add_point(&mut self) {
        self.compaction.error = None;

        let parsed = (|| -> std::result::Result<CompactionPointParts, String> {
            let mold_plus = parse_number(&self.compaction.mold_plus.text, "molde con muestra")?;
            let mold = parse_number(&self.compaction.mold.text, "masa del molde")?;
            let volume = parse_number(&self.compaction.volume.text, "volumen del molde")?;
            let wet_sample = parse_number(&self.compaction.wet_sample.text, "muestra húmeda")?;
            let dry_sample = parse_number(&self.compaction.dry_sample.text, "muestra seca")?;

            let wet = wet_density(mold_plus, mold, volume).map_err(|error| error.to_string())?;
            let moisture = mamut_calculations::compaction::moisture_percent(wet_sample, dry_sample)
                .map_err(|error| error.to_string())?;
            let dry = dry_density(wet, moisture).map_err(|error| error.to_string())?;
            Ok((
                mold, mold_plus, volume, wet_sample, dry_sample, moisture, dry,
            ))
        })();

        match parsed {
            Ok((mold, mold_plus, volume, wet_sample, dry_sample, moisture, dry)) => {
                self.compaction.points.push(CompactionPointView {
                    mold_g: mold,
                    mold_plus_g: mold_plus,
                    volume_cm3: volume,
                    wet_sample_g: wet_sample,
                    dry_sample_g: dry_sample,
                    moisture,
                    dry_density: dry,
                });
                self.compaction.peak = None;
                // Se limpian las masas de la muestra para el siguiente punto, no el molde.
                self.compaction.wet_sample = TextEdit::new("");
                self.compaction.dry_sample = TextEdit::new("");
                self.flash = Some(format!("Punto añadido: {moisture:.1} % · {dry:.3} g/cm³"));
            }
            Err(message) => self.compaction.error = Some(message),
        }
    }

    /// Calcula el máximo de la curva con los puntos acumulados.
    pub fn compaction_calculate(&mut self) {
        self.compaction.error = None;
        self.compaction.peak = None;

        let points: Vec<CompactionPoint> = self
            .compaction
            .points
            .iter()
            .map(|point| CompactionPoint {
                moisture_percent: point.moisture,
                dry_density_g_cm3: point.dry_density,
            })
            .collect();

        match curve_peak(&points) {
            Some(peak) => {
                self.compaction.computed_from = Some(self.compaction_signature());
                self.compaction.peak = Some(PeakOutcome {
                    sample: self.soils.sample.text.trim().to_owned(),
                    optimum_moisture: peak.optimum_moisture_percent,
                    max_dry_density: peak.max_dry_density_g_cm3,
                    points_used: peak.points_used,
                    below_max_measured: peak.below_max_measured,
                });
            }
            None => {
                self.compaction.error = Some(
                    "los puntos no definen un máximo defendible: hacen falta al menos tres y \
                     que la curva suba y baje dentro del rango ensayado"
                        .to_owned(),
                );
            }
        }
    }

    /// Añade una fila a la curva carga–penetración.
    pub fn cbr_add_row(&mut self) {
        self.cbr.rows.push(CbrRow {
            penetration: TextEdit::new(""),
            load: TextEdit::new(""),
        });
    }

    /// Calcula el CBR con la curva introducida.
    pub fn cbr_calculate(&mut self) {
        self.cbr.error = None;
        self.cbr.outcome = None;

        let parsed = (|| -> std::result::Result<Vec<PenetrationPoint>, String> {
            let mut points = Vec::new();
            for (index, row) in self.cbr.rows.iter().enumerate() {
                let penetration = row.penetration.text.trim();
                let load = row.load.text.trim();
                // Una fila existe cuando tiene carga: el formulario trae las penetraciones
                // habituales ya escritas. Carga sin penetración sí es un error.
                if load.is_empty() {
                    continue;
                }
                if penetration.is_empty() {
                    return Err(format!(
                        "la fila {} tiene carga pero no penetración",
                        index + 1
                    ));
                }
                points.push(PenetrationPoint {
                    penetration_mm: parse_number(penetration, "penetración")?,
                    load_mpa: parse_number(load, "carga unitaria")?,
                });
            }
            if points.is_empty() {
                return Err("no hay ningún punto de la curva".to_owned());
            }
            Ok(points)
        })();

        let points = match parsed {
            Ok(points) => points,
            Err(message) => {
                self.cbr.error = Some(message);
                return;
            }
        };

        match cbr_from_curve(&points) {
            Ok(result) => {
                self.cbr.computed_from = Some(self.cbr_signature());
                self.cbr.outcome = Some(CbrOutcome {
                    sample: self.soils.sample.text.trim().to_owned(),
                    points: points
                        .iter()
                        .map(|point| (point.penetration_mm, point.load_mpa))
                        .collect(),
                    percent: result.percent,
                    governing_penetration: result.governing_penetration_mm,
                    offset: result.offset_mm,
                });
            }
            Err(error) => self.cbr.error = Some(error.to_string()),
        }
    }

    /// Vacía los puntos de la curva de compactación.
    pub fn compaction_clear(&mut self) {
        self.compaction.points.clear();
        self.compaction.peak = None;
        self.compaction.error = None;
    }

    /// Guarda el ensayo de suelos y registra el PDF emitido, con su hash.
    ///
    /// Se hace **después** de escribir el PDF porque el registro incluye su SHA-256. Si el
    /// registro falla, el archivo existe pero queda sin constancia: el aviso lo dice en vez
    /// de fingir que todo fue bien.
    fn register_emission(
        &self,
        kind: &'static str,
        descriptor: &'static str,
        inputs_json: &str,
        result_value: f64,
        result_unit: &'static str,
        path: &str,
    ) -> std::result::Result<(), String> {
        let Some(store) = &self.store else {
            return Err("no hay almacenamiento disponible".to_owned());
        };
        let code = self.soils.sample.text.trim().to_owned();
        let actor = Actor::new(LOCAL_ACTOR_ID, "Operador de laboratorio")
            .map_err(|error| error.to_string())?;
        let now = SystemClock.now();
        let performed_on = CivilDate::today(&SystemClock).to_iso();

        let test_id = store.save_soil_test(crate::store::SoilTestDraft {
            code: &code,
            kind,
            descriptor,
            inputs_json,
            result_value,
            result_unit,
            performed_on: &performed_on,
            actor: &actor,
            at: now,
        })?;

        let file = std::path::Path::new(path);
        let bytes = std::fs::read(file).map_err(|error| error.to_string())?;
        store.save_report(&code, kind, &test_id, file, &bytes, &actor, now)?;
        Ok(())
    }

    /// Comprueba que la muestra existe, no está anulada y es la misma para la que se calculó.
    ///
    /// Un informe identifica una muestra concreta: emitirlo para otra, para una anulada o
    /// para un código que no existe produce un documento que afirma algo falso.
    fn require_report_sample(
        &self,
        code: &str,
        outcome_sample: &str,
    ) -> std::result::Result<mamut_domain::Sample, String> {
        if code.is_empty() {
            return Err("indica la muestra a la que corresponde el informe".to_owned());
        }
        if !outcome_sample.is_empty() && outcome_sample != code {
            return Err(format!(
                "el resultado se calculó para «{outcome_sample}», no para «{code}»: \
                 vuelve a calcular con la muestra correcta"
            ));
        }
        let Some(store) = &self.store else {
            return Err("no hay almacenamiento disponible".to_owned());
        };
        match store.find_sample(code) {
            Ok(Some(sample)) => {
                if sample.state == mamut_domain::SampleState::Voided {
                    return Err(format!(
                        "la muestra «{code}» está anulada: no se emiten informes sobre ella"
                    ));
                }
                Ok(sample)
            }
            Ok(None) => Err(format!("no existe la muestra «{code}»")),
            Err(error) => Err(format!("no se pudo leer la muestra: {error}")),
        }
    }

    /// Compone y guarda el informe de un ensayo de suelos.
    ///
    /// Reutiliza la maqueta del informe de compresión: la estructura (identificación, datos
    /// originales, cálculos, resultado) es la misma para todos los ensayos, y tener un solo
    /// renderizador evita que un ensayo se imprima distinto que otro.
    // Los parámetros son los de la ficha del informe: muestra, ensayo, norma, datos
    // originales, cálculos y resultado. Agruparlos en una estructura no los haría más claros.
    #[allow(clippy::too_many_arguments)]
    fn emit_soils_report(
        &mut self,
        file_stem: &str,
        test_name: &str,
        citation: &str,
        outcome_sample: &str,
        measurements: Vec<ReportLine>,
        calculations: Vec<ReportLine>,
        result: ReportLine,
    ) -> std::result::Result<String, String> {
        let code = self.soils.sample.text.trim().to_owned();
        let sample = self.require_report_sample(&code, outcome_sample)?;
        let Some(store) = &self.store else {
            return Err("no hay almacenamiento disponible".to_owned());
        };
        let Some(laboratory) = LaboratoryInfo::new("MAMUT Lab") else {
            return Err("identidad del laboratorio no configurada".to_owned());
        };
        let today = CivilDate::today(&SystemClock);
        // La referencia sirve para el veredicto: sin especificación no hay juicio. Lo que se
        // imprime como «Norma» es el procedimiento que se usó de verdad, con su edición;
        // citar una edición de ASTM no verificada sería inventar un dato en un documento
        // que se firma.
        let norm_ref = NormRef::new(
            NormBody::Internal,
            "PENDIENTE",
            1,
            "verificación normativa pendiente",
            NormStatus::PendingReview,
        )
        .map_err(|error| error.to_string())?;

        let data = ReportData {
            laboratory,
            sample_code: code.clone(),
            client: sample.client.clone(),
            project: sample.project.clone(),
            received_on: sample.received_on,
            test_name: test_name.to_owned(),
            method_label: "Normativo".to_owned(),
            norm_citation: Some(citation.to_owned()),
            performed_on: today,
            operator: sample.responsible.name.clone(),
            equipment: None,
            measurements,
            calculations,
            result,
            // Sin especificación aplicable no hay veredicto: el informe lo dirá.
            conformity: Conformity::evaluate(0.0, &norm_ref, None, None),
            observations: String::new(),
            issued_on: today,
            responsible: sample.responsible.name.clone(),
        };

        let bytes = render_pdf(&data);
        let path = store.save_pdf(&format!("{file_stem}-{code}"), &bytes)?;
        Ok(path.display().to_string())
    }

    /// Emite el informe de límites de Atterberg.
    pub fn report_atterberg(&mut self) {
        self.limits.error = None;
        let Some(outcome) = self.limits.outcome.clone() else {
            self.limits.error = Some("calcula los límites antes de emitir el informe".into());
            return;
        };
        if let Err(message) =
            self.require_fresh(&self.limits_signature(), &self.limits.computed_from)
        {
            self.limits.error = Some(message);
            return;
        }

        // Los datos originales son las determinaciones, no sus derivados: un informe sin
        // ellas no se puede auditar.
        let measurements: Vec<ReportLine> = outcome
            .points
            .iter()
            .map(|(blows, moisture)| {
                ReportLine::with_unit(
                    format!("Determinación a {blows} golpes"),
                    format!("{moisture:.2}"),
                    "%",
                )
            })
            .collect();
        let mut calculations = vec![
            ReportLine::with_unit("Pendiente del ajuste", format!("{:.4}", outcome.slope), ""),
            ReportLine::with_unit(
                "Coeficiente de determinación",
                format!("{:.4}", outcome.r_squared),
                "",
            ),
            ReportLine::with_unit("Determinaciones", outcome.determinations.to_string(), ""),
        ];
        if let Some(plastic) = outcome.plastic_limit {
            calculations.push(ReportLine::with_unit(
                "Límite plástico",
                format!("{plastic:.1}"),
                "%",
            ));
        }
        if let Some(index) = outcome.plasticity_index {
            calculations.push(ReportLine::with_unit(
                "Índice de plasticidad",
                format!("{index:.1}"),
                "",
            ));
        }
        let inputs = serde_json::json!({
            "determinaciones": outcome.points.iter().map(|(blows, moisture)| {
                serde_json::json!({ "golpes": blows, "humedad": moisture })
            }).collect::<Vec<_>>(),
            "agua_g": self.limits.water.text.trim(),
            "seca_g": self.limits.dry.text.trim(),
            "tara_g": self.limits.tare.text.trim(),
        })
        .to_string();
        match self.emit_soils_report(
            "atterberg",
            "Límites de Atterberg",
            "Procedimiento TexDOT Tex-104-E / Tex-105-E (abril 2024) —              equivalencia ASTM D4318 pendiente de verificar",
            &outcome.sample,
            measurements,
            calculations,
            ReportLine::with_unit(
                "Límite líquido",
                format!("{:.1}", outcome.liquid_limit),
                "%",
            ),
        ) {
            Ok(path) => {
                match self.register_emission(
                    "atterberg",
                    "SOIL-ATTERBERG",
                    &inputs,
                    outcome.liquid_limit,
                    "%",
                    &path,
                ) {
                    Ok(()) => self.flash = Some(format!("Informe de Atterberg: {path}")),
                    Err(message) => {
                        self.limits.error = Some(format!(
                            "informe emitido en {path}, pero no quedó registrado: {message}"
                        ))
                    }
                }
            }
            Err(message) => self.limits.error = Some(message),
        }
    }

    /// Emite el informe de compactación.
    pub fn report_compaction(&mut self) {
        self.compaction.error = None;
        if let Err(message) =
            self.require_fresh(&self.compaction_signature(), &self.compaction.computed_from)
        {
            self.compaction.error = Some(message);
            return;
        }
        let Some(peak) = self.compaction.peak.clone() else {
            self.compaction.error = Some("calcula el máximo antes de emitir el informe".into());
            return;
        };
        // Cada punto imprime sus propias pesadas: el informe no puede mostrar un molde o un
        // volumen que ninguno de los puntos usó.
        let points: Vec<ReportLine> = self
            .compaction
            .points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                ReportLine::with_unit(
                    format!("Punto {}", index + 1),
                    format!(
                        "molde {:.1} g · molde+muestra {:.1} g · {:.0} cm³ · húmeda {:.1} g · \
                         seca {:.1} g → {:.1} % · {:.3} g/cm³",
                        point.mold_g,
                        point.mold_plus_g,
                        point.volume_cm3,
                        point.wet_sample_g,
                        point.dry_sample_g,
                        point.moisture,
                        point.dry_density
                    ),
                    "",
                )
            })
            .collect();
        let calculations = vec![ReportLine::with_unit(
            "Humedad óptima",
            format!("{:.1}", peak.optimum_moisture),
            "%",
        )];
        let inputs = serde_json::json!({
            "puntos": self.compaction.points.iter().map(|point| {
                serde_json::json!({
                    "molde_g": point.mold_g,
                    "molde_mas_muestra_g": point.mold_plus_g,
                    "volumen_cm3": point.volume_cm3,
                    "muestra_humeda_g": point.wet_sample_g,
                    "muestra_seca_g": point.dry_sample_g,
                    "humedad": point.moisture,
                    "densidad_seca": point.dry_density,
                })
            }).collect::<Vec<_>>(),
        })
        .to_string();
        match self.emit_soils_report(
            "compactacion",
            "Compactación de laboratorio",
            "Procedimiento TexDOT Tex-113-E (enero 2025) / Tex-114-E —              equivalencia ASTM D698/D1557 pendiente de verificar",
            &peak.sample,
            points,
            calculations,
            ReportLine::with_unit(
                "Densidad seca máxima",
                format!("{:.3}", peak.max_dry_density),
                "g/cm³",
            ),
        ) {
            Ok(path) => match self.register_emission(
                "compactacion",
                "SOIL-COMPACTACION",
                &inputs,
                peak.max_dry_density,
                "g/cm³",
                &path,
            ) {
                Ok(()) => self.flash = Some(format!("Informe de compactación: {path}")),
                Err(message) => {
                    self.compaction.error = Some(format!(
                        "informe emitido en {path}, pero no quedó registrado: {message}"
                    ))
                }
            },
            Err(message) => self.compaction.error = Some(message),
        }
    }

    /// Emite el informe de CBR.
    pub fn report_cbr(&mut self) {
        self.cbr.error = None;
        let Some(outcome) = self.cbr.outcome.clone() else {
            self.cbr.error = Some("calcula el CBR antes de emitir el informe".into());
            return;
        };
        if let Err(message) = self.require_fresh(&self.cbr_signature(), &self.cbr.computed_from) {
            self.cbr.error = Some(message);
            return;
        }

        // La curva medida va al informe: es el dato original del ensayo.
        let measurements: Vec<ReportLine> = outcome
            .points
            .iter()
            .map(|(penetration, load)| {
                ReportLine::with_unit(
                    format!("Penetración {penetration} mm"),
                    format!("{load:.4}"),
                    "MPa",
                )
            })
            .collect();
        let calculations = vec![
            ReportLine::with_unit(
                "Penetración que gobierna",
                format!("{:.2}", outcome.governing_penetration),
                "mm",
            ),
            ReportLine::with_unit(
                "Corrimiento de origen",
                format!("{:.2}", outcome.offset),
                "mm",
            ),
        ];
        let inputs = serde_json::json!({
            "curva": outcome.points.iter().map(|(penetration, load)| {
                serde_json::json!({ "penetracion_mm": penetration, "carga_mpa": load })
            }).collect::<Vec<_>>(),
            "penetracion_gobernante_mm": outcome.governing_penetration,
            "corrimiento_mm": outcome.offset,
        })
        .to_string();
        match self.emit_soils_report(
            "cbr",
            "California Bearing Ratio",
            "Procedimiento ODOT T-193 (mayo 2024) sobre las cargas estándar de              ASTM D1883 — verificación de la norma pendiente",
            &outcome.sample,
            measurements,
            calculations,
            ReportLine::with_unit("CBR", format!("{:.1}", outcome.percent), "%"),
        ) {
            Ok(path) => match self.register_emission(
                "cbr",
                "SOIL-CBR",
                &inputs,
                outcome.percent,
                "%",
                &path,
            ) {
                Ok(()) => self.flash = Some(format!("Informe de CBR: {path}")),
                Err(message) => {
                    self.cbr.error = Some(format!(
                        "informe emitido en {path}, pero no quedó registrado: {message}"
                    ))
                }
            },
            Err(message) => self.cbr.error = Some(message),
        }
    }

    /// Emite el informe de densidad en sitio.
    pub fn report_density(&mut self) {
        self.density.error = None;
        let Some(outcome) = self.density.outcome.clone() else {
            self.density.error = Some("calcula la densidad antes de emitir el informe".into());
            return;
        };
        if let Err(message) =
            self.require_fresh(&self.density_signature(), &self.density.computed_from)
        {
            self.density.error = Some(message);
            return;
        }

        let measurements = vec![
            ReportLine::with_unit(
                "Método de determinación",
                self.density.method.text.trim(),
                "",
            ),
            ReportLine::with_unit("Densidad húmeda", self.density.wet.text.trim(), "g/cm³"),
            ReportLine::with_unit("Humedad en sitio", self.density.moisture.text.trim(), "%"),
            // Sin la referencia de laboratorio, el grado de compactación no se puede
            // reproducir ni auditar.
            ReportLine::with_unit(
                "Densidad seca máxima (laboratorio)",
                self.density.max_dry.text.trim(),
                "g/cm³",
            ),
            ReportLine::with_unit(
                "Humedad óptima (laboratorio)",
                self.density.optimum.text.trim(),
                "%",
            ),
        ];
        let calculations = vec![
            ReportLine::with_unit(
                "Densidad seca",
                format!("{:.3}", outcome.dry_density),
                "g/cm³",
            ),
            ReportLine::with_unit(
                "Desviación de humedad",
                format!("{:+.1}", outcome.moisture_deviation),
                "puntos porcentuales",
            ),
        ];
        let inputs = serde_json::json!({
            "metodo": self.density.method.text.trim(),
            "densidad_humeda": self.density.wet.text.trim(),
            "humedad": self.density.moisture.text.trim(),
            "densidad_maxima": self.density.max_dry.text.trim(),
            "humedad_optima": self.density.optimum.text.trim(),
        })
        .to_string();
        match self.emit_soils_report(
            "densidad",
            "Densidad en sitio",
            "Procedimiento TexDOT Tex-115-E (enero 2025) — equivalencia ASTM D6938/D1556              pendiente de verificar",
            &outcome.sample,
            measurements,
            calculations,
            ReportLine::with_unit(
                "Grado de compactación",
                format!("{:.1}", outcome.relative_compaction),
                "%",
            ),
        ) {
            Ok(path) => match self.register_emission(
                "densidad",
                "SOIL-DENSIDAD",
                &inputs,
                outcome.relative_compaction,
                "%",
                &path,
            ) {
                Ok(()) => self.flash = Some(format!("Informe de densidad: {path}")),
                Err(message) => {
                    self.density.error = Some(format!(
                        "informe emitido en {path}, pero no quedó registrado: {message}"
                    ))
                }
            },
            Err(message) => self.density.error = Some(message),
        }
    }

    /// Firma de los campos del ensayo de compresión: si cambian, el resultado caduca.
    fn test_signature(&self) -> String {
        let form = &self.test_form;
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            form.sample_code.text,
            form.designation.text,
            form.diameter.text,
            form.height.text,
            form.mass.text,
            form.age.text,
            form.load.text
        )
    }

    /// Firma de los campos de granulometría.
    fn soils_signature(&self) -> String {
        let mut out = self.soils.total.text.clone();
        for row in &self.soils.rows {
            out.push('|');
            out.push_str(&row.size.text);
            out.push(':');
            out.push_str(&row.retained.text);
        }
        out
    }

    /// Firma de los campos de límites de Atterberg.
    fn limits_signature(&self) -> String {
        let mut out = String::new();
        for row in &self.limits.rows {
            out.push_str(&row.blows.text);
            out.push(':');
            out.push_str(&row.moisture.text);
            out.push('|');
        }
        out.push_str(&self.limits.water.text);
        out.push('|');
        out.push_str(&self.limits.dry.text);
        out.push('|');
        out.push_str(&self.limits.tare.text);
        out
    }

    /// Firma de los campos de densidad en sitio.
    fn density_signature(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.density.wet.text,
            self.density.moisture.text,
            self.density.max_dry.text,
            self.density.optimum.text
        )
    }

    /// Firma de la curva de compactación: las pesadas de cada punto.
    fn compaction_signature(&self) -> String {
        let mut out = String::new();
        for point in &self.compaction.points {
            out.push_str(&format!(
                "{}:{}:{}:{}:{}|",
                point.mold_g,
                point.mold_plus_g,
                point.volume_cm3,
                point.wet_sample_g,
                point.dry_sample_g
            ));
        }
        out
    }

    /// Firma de la curva de CBR.
    fn cbr_signature(&self) -> String {
        let mut out = String::new();
        for row in &self.cbr.rows {
            out.push_str(&row.penetration.text);
            out.push(':');
            out.push_str(&row.load.text);
            out.push('|');
        }
        out
    }

    /// Comprueba que el resultado mostrado corresponde a lo que hay en pantalla.
    ///
    /// Sin esto, editar un campo después de calcular dejaba un resultado que ya no era el
    /// de los datos visibles: el informe se emitía con la firma de unos números que el
    /// usuario había cambiado.
    fn require_fresh(
        &self,
        current: &str,
        computed: &Option<String>,
    ) -> std::result::Result<(), String> {
        match computed {
            Some(signature) if signature == current => Ok(()),
            _ => Err(
                "los datos cambiaron desde el cálculo: vuelve a calcular antes de guardar o emitir"
                    .to_owned(),
            ),
        }
    }

    /// Carga los informes emitidos de la muestra indicada.
    pub fn load_reports(&mut self) {
        self.reports_error = None;
        self.reports_list.clear();

        let code = self.reports_code.text.trim().to_owned();
        if code.is_empty() {
            self.reports_error = Some("indica la muestra cuyos informes quieres ver".to_owned());
            return;
        }
        let Some(store) = &self.store else {
            self.reports_error = Some("no hay almacenamiento disponible".to_owned());
            return;
        };
        match store.reports_for(&code) {
            Ok(reports) => {
                self.reports_list = reports
                    .iter()
                    .map(|report| {
                        format!(
                            "{} · {} · {} · {}",
                            report.kind,
                            report.issued_at,
                            &report.sha256[..12],
                            report.path
                        )
                    })
                    .collect();
                if self.reports_list.is_empty() {
                    self.reports_error =
                        Some(format!("la muestra «{code}» no tiene informes emitidos"));
                }
            }
            Err(error) => self.reports_error = Some(error),
        }
    }

    /// Texto de la barra de estado.
    ///
    /// Es una función del estado y no de la vista, para poder probar que **dice lo que
    /// debe** sin depender de cómo se dibuja.
    pub fn status_line(&self) -> String {
        format!(
            "{} muestras · {} ensayos · normas: {} verificadas, {} pendientes ({}) · {} · {}",
            self.samples_total,
            self.tests_total,
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
    use mamut_core::Timestamp;
    use mamut_domain::SampleState;

    fn row(code: &str) -> SampleRow {
        SampleRow {
            code: code.into(),
            material: "Hormigón".into(),
            client: "Andina".into(),
            received_on: "2026-11-18".into(),
            state: "Recibida".into(),
        }
    }

    fn ts() -> Timestamp {
        Timestamp::from_millis(1_763_500_000_000)
    }

    fn state_with_store() -> AppState {
        AppState::with_store(Store::in_memory(ts()).unwrap())
    }

    fn fill(form: &mut SampleForm, code: &str, date: &str) {
        form.code = TextEdit::new(code);
        form.origin = TextEdit::new("Bloque 3");
        form.client = TextEdit::new("Constructora Andina");
        form.project = TextEdit::new("Torre Norte");
        form.received_on = TextEdit::new(date);
        form.responsible = TextEdit::new("Ana Torres");
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
        state.samples_total = 7;
        state.tests_total = 3;
        state.norms_verified = 2;
        state.norms_pending = 5;
        state.pack_source = "archivo: propio.json".into();
        state.storage_status = "base local".into();

        let line = state.status_line();
        assert!(line.contains("7 muestras"), "{line}");
        assert!(line.contains("3 ensayos"), "{line}");
        assert!(line.contains("2 verificadas"), "{line}");
        assert!(line.contains("5 pendientes"), "{line}");
        assert!(line.contains("archivo: propio.json"), "{line}");
    }

    #[test]
    fn abrir_el_formulario_pone_la_fecha_de_hoy() {
        let mut state = state_with_store();
        state.open_form();
        assert!(state.form_open);
        assert_eq!(
            state.form.received_on.text.len(),
            10,
            "la fecha se rellena en formato AAAA-MM-DD"
        );
    }

    #[test]
    fn guardar_una_muestra_valida_la_persiste_y_limpia_el_formulario() {
        let mut state = state_with_store();
        state.open_form();
        fill(&mut state.form, "H-2026-0042", "2026-11-20");
        state.save_form();

        assert!(state.form.error.is_none(), "{:?}", state.form.error);
        assert!(!state.form_open, "el formulario se cierra al guardar");
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.rows[0].code, "H-2026-0042");
        assert_eq!(state.samples_total, 1);
        assert!(state.flash.as_deref().unwrap_or("").contains("H-2026-0042"));
        assert!(state.form.code.text.is_empty(), "el borrador se limpia");
    }

    #[test]
    fn un_codigo_vacio_no_guarda_nada() {
        let mut state = state_with_store();
        state.open_form();
        fill(&mut state.form, "   ", "2026-11-20");
        state.save_form();

        assert!(state.form.error.is_some());
        assert!(state.form_open, "el formulario sigue abierto para corregir");
        assert_eq!(state.samples_total, 0);
        assert!(state.rows.is_empty());
    }

    #[test]
    fn una_fecha_invalida_no_guarda_nada() {
        let mut state = state_with_store();
        state.open_form();
        fill(&mut state.form, "H-2026-0043", "2026-02-31");
        state.save_form();

        assert!(
            state.form.error.as_deref().unwrap_or("").contains("fecha"),
            "el mensaje debe nombrar el campo: {:?}",
            state.form.error
        );
        assert_eq!(state.samples_total, 0);
    }

    #[test]
    fn un_cliente_vacio_no_guarda_nada() {
        let mut state = state_with_store();
        state.open_form();
        fill(&mut state.form, "H-2026-0044", "2026-11-20");
        state.form.client = TextEdit::new("   ");
        state.save_form();
        assert!(state.form.error.is_some());
        assert_eq!(state.samples_total, 0);
    }

    #[test]
    fn dos_muestras_con_el_mismo_codigo_muestran_el_error() {
        let mut state = state_with_store();
        for _ in 0..2 {
            state.open_form();
            fill(&mut state.form, "H-2026-0045", "2026-11-20");
            state.save_form();
        }
        assert_eq!(state.samples_total, 1);
        assert!(
            state.form.error.is_some(),
            "el segundo intento debe explicar por qué no se guardó"
        );
    }

    #[test]
    fn cancelar_el_formulario_no_guarda_nada() {
        let mut state = state_with_store();
        state.open_form();
        fill(&mut state.form, "H-2026-0046", "2026-11-20");
        state.cancel_form();
        assert!(!state.form_open);
        assert_eq!(state.samples_total, 0);
        assert!(state.form.code.text.is_empty());
    }

    #[test]
    fn el_material_elegido_llega_a_la_muestra() {
        let mut state = state_with_store();
        state.open_form();
        fill(&mut state.form, "S-2026-0001", "2026-11-20");
        state.set_material(Material::Soil);
        state.save_form();
        assert_eq!(state.rows[0].material, "Suelo");
    }

    #[test]
    fn sin_almacenamiento_el_formulario_avisa_en_vez_de_fingir() {
        let mut state = AppState::new(vec![]);
        state.open_form();
        fill(&mut state.form, "H-2026-0047", "2026-11-20");
        state.save_form();
        assert!(
            state
                .form
                .error
                .as_deref()
                .unwrap_or("")
                .contains("almacenamiento"),
            "{:?}",
            state.form.error
        );
        assert_eq!(state.samples_total, 0);
    }

    #[test]
    fn una_fila_se_formatea_a_partir_de_la_muestra() {
        let sample = Sample::new(
            SampleCode::new("H-2026-0001").unwrap(),
            Material::Concrete,
            "Bloque 3",
            "Constructora Andina",
            "Torre Norte",
            CivilDate::new(2026, 11, 18).unwrap(),
            Actor::new("u-1", "Ana Torres").unwrap(),
            ts(),
        )
        .unwrap();

        let row = SampleRow::from_sample(&sample);
        assert_eq!(row.code, "H-2026-0001");
        assert_eq!(row.material, "Hormigón");
        assert_eq!(row.received_on, "2026-11-18");
        assert_eq!(row.state, "Recibida");
        assert_eq!(sample.state, SampleState::Received);
    }
}
