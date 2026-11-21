//! Estado de la interfaz.
//!
//! Es **solo** lo que la pantalla necesita para dibujarse y para atender sus acciones.
//! Las reglas de negocio viven en `mamut-domain`; el acceso a datos, en [`Store`].
//!
//! La vista no modifica este estado: lo hacen los manejadores (`.on_press`, `.on_edit`),
//! que es lo que mantiene la interfaz predecible y probable.

use std::cell::Cell;

use mamut_calculations::{CylinderCompressionInput, ENGINE_VERSION, RoundingPolicy, calculate};
use mamut_core::{Actor, CivilDate, Clock, Force, Length, Mass, SystemClock, Timestamp};
use mamut_domain::{Material, Measurement, ResultRef, Sample, SampleCode, Specimen, TestRecord};
use mamut_reports::{LaboratoryInfo, ReportData, ReportLine, render_pdf, verdict_text};
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
        }
    }

    /// Limpia el resultado, pero **no** los datos: corregir una carga no debe obligar a
    /// reescribir las dimensiones.
    pub fn clear_result(&mut self) {
        self.outcome = None;
        self.input = None;
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
            test_history: Vec::new(),
            descriptor: None,
            tool_code: TextEdit::new(""),
            tool_path: TextEdit::new(""),
            tool_reason: TextEdit::new(""),
            attachments: Vec::new(),
            series: Vec::new(),
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

            // Los datos originales se guardan tal como se teclearon.
            let measurements = vec![
                Measurement::new("diametro", self.test_form.diameter.text.trim(), "mm", now)
                    .map_err(|error| error.to_string())?,
                Measurement::new("altura", self.test_form.height.text.trim(), "mm", now)
                    .map_err(|error| error.to_string())?,
                Measurement::new("masa", self.test_form.mass.text.trim(), "g", now)
                    .map_err(|error| error.to_string())?,
                Measurement::new("carga_maxima", self.test_form.load.text.trim(), "kN", now)
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
                self.test_form.saved = Some(test_id);
                self.flash = Some(format!("Ensayo guardado para la muestra {code}"));
                self.load_test_history();
                // Las herramientas (adjuntos y gráfico) siguen a la muestra ensayada.
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
                self.test_form.report_path = Some(path.display().to_string());
                self.flash = Some(format!("Informe generado: {}", path.display()));
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
                        designation: test.descriptor.code.as_str().to_owned(),
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
                            "{} · {} KB · {}…",
                            record.file_name,
                            record.bytes / 1024,
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
            Ok(path) => self.flash = Some(format!("Exportado: {}", path.display())),
            Err(message) => self.flash = Some(message),
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
