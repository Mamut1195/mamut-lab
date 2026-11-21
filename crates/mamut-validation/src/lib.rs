//! `mamut-validation` — comprobaciones de entrada.
//!
//! **Qué es y qué no es.** Estas son comprobaciones de **plausibilidad física**, no
//! criterios normativos. Sirven para que un cero de más o un diámetro de 15 mm no llegue
//! a un informe; no dicen si un hormigón cumple.
//!
//! Los rangos son configurables y llevan su justificación al lado. Cuando la revisión de
//! una norma fije un criterio, ese criterio vivirá en el *norm pack* (ADR-0009), no aquí.
//!
//! Se distinguen tres severidades y la distinción importa: un `Error` impide continuar;
//! un `Warning` deja continuar pero avisa; un `Info` solo informa.

use mamut_calculations::CylinderCompressionInput;
use mamut_core::Area;

/// Gravedad de un hallazgo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "información",
            Self::Warning => "aviso",
            Self::Error => "error",
        }
    }
}

/// Un hallazgo concreto sobre un dato.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// Código estable, para pruebas y para la interfaz.
    pub code: &'static str,
    pub severity: Severity,
    /// Campo al que se refiere (`diametro`, `masa`, …).
    pub field: &'static str,
    pub message: String,
}

impl Finding {
    pub fn error(code: &'static str, field: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            field,
            message: message.into(),
        }
    }

    pub fn warning(code: &'static str, field: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            field,
            message: message.into(),
        }
    }

    pub fn info(code: &'static str, field: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Info,
            field,
            message: message.into(),
        }
    }
}

/// Conjunto de hallazgos de una validación.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidationReport {
    pub findings: Vec<Finding>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        !self.has_errors()
    }

    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
    }

    pub fn has_code(&self, code: &str) -> bool {
        self.findings.iter().any(|f| f.code == code)
    }

    fn push(&mut self, finding: Finding) {
        self.findings.push(finding);
    }
}

/// Rangos de plausibilidad. **No son límites normativos.**
///
/// Cada valor tiene un motivo físico o de sentido común; ninguno se presenta como
/// criterio de aceptación.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlausibilityLimits {
    /// Diámetro habitual de probeta de hormigón, en mm. Fuera de esto, probable error
    /// de tecleo o de unidad (cm en vez de mm).
    pub diameter_mm: (f64, f64),
    /// Altura habitual, en mm.
    pub height_mm: (f64, f64),
    /// Esbeltez (altura/diámetro) habitual. Aviso, no rechazo.
    pub slenderness: (f64, f64),
    /// Densidad aparente del hormigón, en kg/m³. Rango físico amplio.
    pub density_kg_m3: (f64, f64),
    /// Edad del espécimen, en días.
    pub age_days: (u32, u32),
}

impl Default for PlausibilityLimits {
    fn default() -> Self {
        Self {
            diameter_mm: (50.0, 300.0),
            height_mm: (50.0, 400.0),
            slenderness: (1.5, 2.5),
            density_kg_m3: (1_500.0, 3_000.0),
            age_days: (1, 3_650),
        }
    }
}

/// Valida una entrada de compresión de cilindros.
///
/// Los errores son imposibilidades; los avisos, rarezas que el operador debe mirar.
pub fn validate_cylinder_input(
    input: &CylinderCompressionInput,
    limits: &PlausibilityLimits,
) -> ValidationReport {
    let mut report = ValidationReport::default();

    // --- imposibilidades ---
    if !input.diameter.is_positive() {
        report.push(Finding::error(
            "E-DIA-POS",
            "diametro",
            "el diámetro debe ser mayor que cero",
        ));
    }
    if !input.height.is_positive() {
        report.push(Finding::error(
            "E-ALT-POS",
            "altura",
            "la altura debe ser mayor que cero",
        ));
    }
    if !input.mass.is_positive() {
        report.push(Finding::error(
            "E-MAS-POS",
            "masa",
            "la masa debe ser mayor que cero",
        ));
    }
    if !input.max_load.is_positive() {
        report.push(Finding::error(
            "E-CAR-POS",
            "carga_maxima",
            "la carga máxima debe ser mayor que cero",
        ));
    }

    // Si falta lo esencial, no tiene sentido seguir comparando rangos.
    if report.has_errors() {
        return report;
    }

    // --- plausibilidad ---
    let diameter = input.diameter.millimetres_value();
    if diameter < limits.diameter_mm.0 || diameter > limits.diameter_mm.1 {
        report.push(Finding::warning(
            "W-DIA-RANGO",
            "diametro",
            format!(
                "diámetro fuera del rango habitual ({:.0}–{:.0} mm): {diameter:.1} mm. Verifique la unidad.",
                limits.diameter_mm.0, limits.diameter_mm.1
            ),
        ));
    }

    let height = input.height.millimetres_value();
    if height < limits.height_mm.0 || height > limits.height_mm.1 {
        report.push(Finding::warning(
            "W-ALT-RANGO",
            "altura",
            format!(
                "altura fuera del rango habitual ({:.0}–{:.0} mm): {height:.1} mm. Verifique la unidad.",
                limits.height_mm.0, limits.height_mm.1
            ),
        ));
    }

    let slenderness = height / diameter;
    if slenderness < limits.slenderness.0 || slenderness > limits.slenderness.1 {
        report.push(Finding::warning(
            "W-ESBELTEZ",
            "altura",
            format!(
                "relación altura/diámetro {slenderness:.2} fuera de lo habitual ({:.1}–{:.1}). Compruebe las dimensiones.",
                limits.slenderness.0, limits.slenderness.1
            ),
        ));
    }

    if let Ok(area) = Area::of_circle(input.diameter) {
        let volume = area.canonical() * input.height.canonical();
        if volume > 0.0 {
            let density = input.mass.canonical() / volume;
            if density < limits.density_kg_m3.0 || density > limits.density_kg_m3.1 {
                report.push(Finding::warning(
                    "W-DENSIDAD",
                    "masa",
                    format!(
                        "densidad aparente {density:.0} kg/m³ fuera del rango físico esperado ({:.0}–{:.0}). Verifique masa y dimensiones.",
                        limits.density_kg_m3.0, limits.density_kg_m3.1
                    ),
                ));
            }
        }
    }

    if input.age_days < limits.age_days.0 || input.age_days > limits.age_days.1 {
        report.push(Finding::warning(
            "W-EDAD",
            "edad",
            format!(
                "edad de {} días fuera del rango habitual ({}–{} días).",
                input.age_days, limits.age_days.0, limits.age_days.1
            ),
        ));
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use mamut_calculations::RoundingPolicy;
    use mamut_core::{CivilDate, Force, Length, Mass};
    use mamut_standards::{NormBody, NormRef, NormStatus};

    fn norm() -> NormRef {
        NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2026,
            "Compresión",
            NormStatus::PendingReview,
        )
        .unwrap()
    }

    fn input(
        diameter_mm: f64,
        height_mm: f64,
        mass_g: f64,
        load_kn: f64,
    ) -> CylinderCompressionInput {
        CylinderCompressionInput {
            diameter: Length::millimetres(diameter_mm).unwrap(),
            height: Length::millimetres(height_mm).unwrap(),
            mass: Mass::grams(mass_g).unwrap(),
            max_load: Force::kilonewtons(load_kn).unwrap(),
            age_days: 28,
            norm: norm(),
            rounding: RoundingPolicy::PROVISIONAL,
        }
    }

    fn limits() -> PlausibilityLimits {
        PlausibilityLimits::default()
    }

    #[test]
    fn un_cilindro_normal_no_produce_hallazgos() {
        let report = validate_cylinder_input(&input(150.0, 300.0, 12_300.0, 500.0), &limits());
        assert!(
            report.findings.is_empty(),
            "un caso normal no debe generar ruido: {:?}",
            report.findings
        );
        assert!(report.is_valid());
    }

    #[test]
    fn un_diametro_cero_es_un_error() {
        let report = validate_cylinder_input(&input(0.0, 300.0, 12_300.0, 500.0), &limits());
        assert!(report.has_code("E-DIA-POS"));
        assert!(!report.is_valid());
    }

    #[test]
    fn una_carga_cero_es_un_error() {
        let report = validate_cylinder_input(&input(150.0, 300.0, 12_300.0, 0.0), &limits());
        assert!(report.has_code("E-CAR-POS"));
    }

    #[test]
    fn un_diametro_en_centimetros_avisa_por_la_unidad() {
        // 15 en vez de 150 mm: el error de unidad más común.
        let report = validate_cylinder_input(&input(15.0, 30.0, 12_300.0, 500.0), &limits());
        assert!(report.has_code("W-DIA-RANGO"));
        assert!(report.is_valid(), "es un aviso, no un error");
    }

    #[test]
    fn una_densidad_imposible_avisa() {
        // 123 kg en un cilindro de 150x300: densidad ~10 000 kg/m³.
        let report = validate_cylinder_input(&input(150.0, 300.0, 123_000.0, 500.0), &limits());
        assert!(report.has_code("W-DENSIDAD"));
        assert!(report.is_valid());
    }

    #[test]
    fn una_esbeltez_anomala_avisa() {
        let report = validate_cylinder_input(&input(150.0, 200.0, 8_200.0, 400.0), &limits());
        assert!(report.has_code("W-ESBELTEZ"));
    }

    #[test]
    fn los_limites_son_configurables() {
        let mut l = limits();
        l.diameter_mm = (100.0, 200.0);
        // 150 mm entra en el rango por defecto pero también en este.
        assert!(
            validate_cylinder_input(&input(150.0, 300.0, 12_300.0, 500.0), &l)
                .findings
                .is_empty()
        );
        l.diameter_mm = (100.0, 120.0);
        assert!(
            validate_cylinder_input(&input(150.0, 300.0, 12_300.0, 500.0), &l)
                .has_code("W-DIA-RANGO")
        );
    }

    #[test]
    fn la_severidad_se_ordena_y_se_etiqueta() {
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
        assert_eq!(Severity::Error.label(), "error");
    }

    #[test]
    fn el_informe_separa_errores_de_avisos() {
        let report = validate_cylinder_input(&input(15.0, 30.0, 12_300.0, 500.0), &limits());
        assert_eq!(report.errors().count(), 0);
        assert!(report.warnings().count() >= 1);
    }

    #[test]
    fn no_se_declara_nada_sobre_conformidad() {
        // Blindaje conceptual: la validación no conoce normas ni veredictos.
        let report = validate_cylinder_input(&input(150.0, 300.0, 12_300.0, 500.0), &limits());
        assert!(
            report
                .findings
                .iter()
                .all(|f| !f.message.to_lowercase().contains("conforme")),
            "la validación no puede emitir juicios de conformidad"
        );
    }

    #[test]
    fn las_fechas_no_intervienen_en_la_validacion() {
        // La edad es un número de días; la fecha de ensayo la aporta el dominio.
        let _ = CivilDate::new(2026, 11, 18).unwrap();
        assert!(
            validate_cylinder_input(&input(150.0, 300.0, 12_300.0, 500.0), &limits()).is_valid()
        );
    }
}
