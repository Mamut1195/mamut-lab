//! `mamut-reports` — composición de informes técnicos.
//!
//! **Fase 1 entrega el modelo y el renderizador de texto**, no el PDF. El PDF llega en
//! la fase 2 (ADR-0005) y se enchufa implementando [`ReportRenderer`]; la composición
//! del contenido —qué secciones existen, en qué orden y con qué avisos— ya está aquí y
//! es lo que se prueba.
//!
//! **Regla que el renderizador no puede saltarse:** el informe imprime el veredicto que
//! recibe. Si el veredicto es «no evaluado», el informe dice **sin juicio de
//! conformidad** y su motivo. No hay ninguna ruta de código que convierta un resultado
//! calculado en un «conforme» por omisión.

pub mod layout;
pub mod pdf;

pub use layout::render_pdf;
pub use pdf::PdfDocument;

use mamut_core::{CivilDate, ReportId};
use mamut_standards::Conformity;

/// Identidad del laboratorio que emite el informe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaboratoryInfo {
    pub name: String,
    pub accreditation: Option<String>,
    pub address: Option<String>,
}

impl LaboratoryInfo {
    pub fn new(name: impl Into<String>) -> Option<Self> {
        let name = name.into().trim().to_owned();
        if name.is_empty() {
            return None;
        }
        Some(Self {
            name,
            accreditation: None,
            address: None,
        })
    }
}

/// Una línea de datos con su unidad explícita.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportLine {
    pub label: String,
    pub value: String,
    pub unit: Option<String>,
}

impl ReportLine {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            unit: None,
        }
    }

    pub fn with_unit(
        label: impl Into<String>,
        value: impl Into<String>,
        unit: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            unit: Some(unit.into()),
        }
    }

    fn formatted(&self) -> String {
        match &self.unit {
            Some(unit) if !unit.is_empty() => format!("{} {}", self.value, unit),
            _ => self.value.clone(),
        }
    }
}

/// Todo lo que un informe necesita. Es un objeto de valor: no conoce la base de datos ni
/// la interfaz, así que se puede construir en una prueba sin infraestructura.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportData {
    pub laboratory: LaboratoryInfo,
    pub sample_code: String,
    pub client: String,
    pub project: String,
    pub received_on: CivilDate,
    pub test_name: String,
    pub method_label: String,
    pub norm_citation: Option<String>,
    pub performed_on: CivilDate,
    pub operator: String,
    pub equipment: Option<String>,
    pub measurements: Vec<ReportLine>,
    pub calculations: Vec<ReportLine>,
    pub result: ReportLine,
    pub conformity: Conformity,
    pub observations: String,
    pub issued_on: CivilDate,
    pub responsible: String,
}

impl ReportData {
    pub fn report_id(&self) -> ReportId {
        ReportId::new()
    }

    /// Si el informe puede afirmar algo sobre el cumplimiento.
    pub fn states_a_verdict(&self) -> bool {
        self.conformity.is_evaluated()
    }
}

/// Contrato de renderizado. El PDF de la fase 2 será otra implementación.
pub trait ReportRenderer {
    fn render(&self, data: &ReportData) -> String;
}

/// Renderizador de texto plano: previsualización y pruebas.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlainTextRenderer;

const RULE: &str = "------------------------------------------------------------";

impl ReportRenderer for PlainTextRenderer {
    fn render(&self, data: &ReportData) -> String {
        let mut out = String::new();
        let line = |out: &mut String, text: &str| {
            out.push_str(text);
            out.push('\n');
        };

        line(
            &mut out,
            "============================================================",
        );
        line(&mut out, "INFORME DE ENSAYO");
        line(
            &mut out,
            "============================================================",
        );
        line(&mut out, &format!("Laboratorio: {}", data.laboratory.name));
        if let Some(accreditation) = &data.laboratory.accreditation {
            line(&mut out, &format!("Acreditación: {accreditation}"));
        }
        if let Some(address) = &data.laboratory.address {
            line(&mut out, &format!("Dirección: {address}"));
        }
        line(&mut out, &format!("Emitido: {}", data.issued_on));
        line(&mut out, RULE);

        line(&mut out, "1. IDENTIFICACIÓN DE LA MUESTRA");
        line(&mut out, &format!("   Muestra: {}", data.sample_code));
        line(&mut out, &format!("   Cliente: {}", data.client));
        line(&mut out, &format!("   Proyecto: {}", data.project));
        line(&mut out, &format!("   Recepción: {}", data.received_on));
        line(&mut out, RULE);

        line(&mut out, "2. ENSAYO");
        line(&mut out, &format!("   Ensayo: {}", data.test_name));
        line(&mut out, &format!("   Método: {}", data.method_label));
        match &data.norm_citation {
            Some(citation) => line(&mut out, &format!("   Norma: {citation}")),
            None => line(&mut out, "   Norma: sin referencia normativa asociada"),
        }
        line(
            &mut out,
            &format!("   Fecha del ensayo: {}", data.performed_on),
        );
        line(&mut out, &format!("   Operador: {}", data.operator));
        if let Some(equipment) = &data.equipment {
            line(&mut out, &format!("   Equipo: {equipment}"));
        }
        line(&mut out, RULE);

        line(&mut out, "3. DATOS ORIGINALES");
        if data.measurements.is_empty() {
            line(&mut out, "   (sin mediciones registradas)");
        } else {
            for measurement in &data.measurements {
                line(
                    &mut out,
                    &format!("   {}: {}", measurement.label, measurement.formatted()),
                );
            }
        }
        line(&mut out, RULE);

        line(&mut out, "4. CÁLCULOS");
        if data.calculations.is_empty() {
            line(&mut out, "   (sin cálculos derivados)");
        } else {
            for calculation in &data.calculations {
                line(
                    &mut out,
                    &format!("   {}: {}", calculation.label, calculation.formatted()),
                );
            }
        }
        line(&mut out, RULE);

        line(&mut out, "5. RESULTADO");
        line(
            &mut out,
            &format!("   {}: {}", data.result.label, data.result.formatted()),
        );
        line(
            &mut out,
            &format!("   Conformidad: {}", verdict_text(&data.conformity)),
        );
        line(&mut out, RULE);

        line(&mut out, "6. OBSERVACIONES");
        if data.observations.trim().is_empty() {
            line(&mut out, "   (sin observaciones)");
        } else {
            for observation in data.observations.lines() {
                line(&mut out, &format!("   {observation}"));
            }
        }
        line(&mut out, RULE);
        line(
            &mut out,
            &format!("Responsable técnico: {}", data.responsible),
        );
        line(
            &mut out,
            "============================================================",
        );

        out
    }
}

/// Texto del veredicto, compartido por el renderizador de texto y el PDF.
///
/// Es la única función que convierte un [`Conformity`] en palabras, de modo que pantalla,
/// previsualización e informe no puedan decir cosas distintas.
pub fn verdict_text(conformity: &Conformity) -> String {
    match conformity {
        Conformity::Conforms { spec, .. } => {
            format!("CONFORME — {}: {}", spec.source, spec.requirement)
        }
        Conformity::DoesNotConform { spec, reason, .. } => match reason {
            mamut_standards::FailureReason::BelowMinimum { limit, actual } => format!(
                "NO CONFORME — {}: {} (valor {actual:.4} por debajo del mínimo {limit})",
                spec.source, spec.requirement
            ),
            mamut_standards::FailureReason::AboveMaximum { limit, actual } => format!(
                "NO CONFORME — {}: {} (valor {actual:.4} por encima del máximo {limit})",
                spec.source, spec.requirement
            ),
        },
        Conformity::NotEvaluated { reason } => {
            format!("SIN JUICIO DE CONFORMIDAD — {}", reason.label())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mamut_standards::{Conformity, NormBody, NormRef, NormStatus, SpecLimits, SpecRef};

    fn date() -> CivilDate {
        CivilDate::new(2026, 11, 20).unwrap()
    }

    fn pending_norm() -> NormRef {
        NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2026,
            "Compresión de cilindros",
            NormStatus::PendingReview,
        )
        .unwrap()
    }

    fn verified_norm() -> NormRef {
        NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2026,
            "Compresión de cilindros",
            NormStatus::Verified {
                reviewed_by: "Ing. Ana Torres".into(),
                reviewed_on: date(),
            },
        )
        .unwrap()
    }

    fn base(conformity: Conformity) -> ReportData {
        ReportData {
            laboratory: LaboratoryInfo::new("Laboratorio Central").unwrap(),
            sample_code: "H-2026-0001".into(),
            client: "Constructora Andina".into(),
            project: "Torre Norte".into(),
            received_on: CivilDate::new(2026, 11, 1).unwrap(),
            test_name: "Compresión de cilindros".into(),
            method_label: "Normativo".into(),
            norm_citation: Some("ASTM C39/C39M-26".into()),
            performed_on: CivilDate::new(2026, 11, 18).unwrap(),
            operator: "Ana Torres".into(),
            equipment: Some("Máquina universal 1".into()),
            measurements: vec![
                ReportLine::with_unit("Diámetro", "150.0", "mm"),
                ReportLine::with_unit("Carga máxima", "500.0", "kN"),
            ],
            calculations: vec![ReportLine::with_unit("Área", "17671.46", "mm²")],
            result: ReportLine::with_unit("Resistencia", "28.29", "MPa"),
            conformity,
            observations: String::new(),
            issued_on: date(),
            responsible: "Ing. Ana Torres".into(),
        }
    }

    fn not_evaluated() -> Conformity {
        Conformity::evaluate(28.29, &pending_norm(), None, None)
    }

    #[test]
    fn el_informe_incluye_todas_las_secciones_exigidas() {
        let text = PlainTextRenderer.render(&base(not_evaluated()));
        for expected in [
            "IDENTIFICACIÓN DE LA MUESTRA",
            "ENSAYO",
            "DATOS ORIGINALES",
            "CÁLCULOS",
            "RESULTADO",
            "OBSERVACIONES",
            "Responsable técnico",
        ] {
            assert!(text.contains(expected), "falta la sección «{expected}»");
        }
    }

    #[test]
    fn el_informe_lleva_la_norma_y_su_version() {
        let text = PlainTextRenderer.render(&base(not_evaluated()));
        assert!(
            text.contains("ASTM C39/C39M-26"),
            "la versión debe aparecer"
        );
    }

    #[test]
    fn los_datos_originales_aparecen_con_unidad() {
        let text = PlainTextRenderer.render(&base(not_evaluated()));
        assert!(text.contains("Diámetro: 150.0 mm"));
        assert!(text.contains("Carga máxima: 500.0 kN"));
        assert!(text.contains("Área: 17671.46 mm²"));
        assert!(text.contains("Resistencia: 28.29 MPa"));
    }

    #[test]
    fn sin_juicio_el_informe_lo_dice_y_no_afirma_conformidad() {
        let text = PlainTextRenderer.render(&base(not_evaluated()));
        assert!(text.contains("SIN JUICIO DE CONFORMIDAD"));
        assert!(
            !text.contains("Conformidad: CONFORME"),
            "no puede aparecer un veredicto que no existe"
        );
        assert!(!text.contains("RESULTADO: CONFORME"));
    }

    #[test]
    fn el_informe_no_oculta_el_motivo_de_no_dictaminar() {
        let text = PlainTextRenderer.render(&base(not_evaluated()));
        assert!(
            text.contains("pendiente de revisión"),
            "el motivo debe ser legible para el cliente"
        );
    }

    #[test]
    fn con_norma_revisada_y_especificacion_si_hay_veredicto() {
        let spec = SpecRef::new("NEC 2015", "f'c de diseño").unwrap();
        let limits = SpecLimits::new(Some(21.0), None).unwrap();
        let conformity = Conformity::evaluate(28.29, &verified_norm(), Some(&spec), Some(&limits));
        let text = PlainTextRenderer.render(&base(conformity));
        assert!(text.contains("Conformidad: CONFORME — NEC 2015"));
        assert!(!text.contains("SIN JUICIO"));
    }

    #[test]
    fn un_resultado_bajo_el_minimo_se_informa_como_no_conforme() {
        let spec = SpecRef::new("NEC 2015", "f'c de diseño").unwrap();
        let limits = SpecLimits::new(Some(30.0), None).unwrap();
        let conformity = Conformity::evaluate(28.29, &verified_norm(), Some(&spec), Some(&limits));
        let text = PlainTextRenderer.render(&base(conformity));
        assert!(text.contains("NO CONFORME"));
        assert!(text.contains("por debajo del mínimo 30"));
    }

    #[test]
    fn un_ensayo_sin_norma_lo_declara() {
        let mut data = base(not_evaluated());
        data.norm_citation = None;
        let text = PlainTextRenderer.render(&data);
        assert!(text.contains("sin referencia normativa asociada"));
    }

    #[test]
    fn las_observaciones_se_imprimen_por_lineas() {
        let mut data = base(not_evaluated());
        data.observations = "Primera nota.\nSegunda nota.".into();
        let text = PlainTextRenderer.render(&data);
        assert!(text.contains("Primera nota."));
        assert!(text.contains("Segunda nota."));
    }

    #[test]
    fn states_a_verdict_refleja_la_puerta_de_conformidad() {
        assert!(!base(not_evaluated()).states_a_verdict());
    }
}
