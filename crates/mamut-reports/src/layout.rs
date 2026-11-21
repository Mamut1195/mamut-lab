//! Maquetación del informe técnico sobre el generador PDF.
//!
//! Una sola columna, A4, márgenes de 18 mm y paginación automática. La estructura es la
//! que exige el producto: identificación, ensayo, datos originales, cálculos, resultado,
//! conformidad y observaciones. **El veredicto se imprime tal cual llega**: si no se
//! evaluó, el informe lo dice y no hay ninguna rama que lo convierta en «conforme».

use crate::pdf::PdfDocument;
use crate::{ReportData, verdict_text};

const LEFT: f32 = 18.0;
const RIGHT: f32 = 192.0;
const WIDTH: f32 = RIGHT - LEFT;
const TOP: f32 = 18.0;
const BOTTOM: f32 = 276.0;
const LABEL_X: f32 = LEFT + 62.0;
const LINE: f32 = 5.4;

struct Layout {
    doc: PdfDocument,
    y: f32,
    page: usize,
    /// Cabecera compacta que se repite en las páginas siguientes.
    running_title: String,
}

impl Layout {
    fn new(title: &str, running_title: &str) -> Self {
        Self {
            doc: PdfDocument::new(title),
            y: TOP,
            page: 1,
            running_title: running_title.to_owned(),
        }
    }

    fn space(&mut self, needed: f32) {
        if self.y + needed > BOTTOM {
            self.footer();
            self.doc.add_page();
            self.page += 1;
            self.y = TOP;
            let title = self.running_title.clone();
            self.doc.text(LEFT, self.y, 8.0, false, &title);
            self.y += 4.0;
            self.doc.line(LEFT, self.y, RIGHT, self.y);
            self.y += 6.0;
        }
    }

    fn footer(&mut self) {
        let page = self.page;
        self.doc.line(LEFT, BOTTOM + 2.0, RIGHT, BOTTOM + 2.0);
        self.doc.text(
            LEFT,
            BOTTOM + 7.0,
            7.5,
            false,
            &format!("MAMUT Lab · informe técnico · página {page}"),
        );
    }

    fn heading(&mut self, text: &str) {
        self.space(12.0);
        self.y += 2.0;
        self.doc.text(LEFT, self.y, 9.5, true, text);
        self.y += 2.0;
        self.doc.line(LEFT, self.y, RIGHT, self.y);
        self.y += 5.0;
    }

    fn row(&mut self, label: &str, value: &str) {
        self.space(LINE);
        self.doc.text(LEFT, self.y, 8.5, false, label);
        self.doc.text(LABEL_X, self.y, 8.5, false, value);
        self.y += LINE;
    }

    fn paragraph(&mut self, text: &str) {
        for line in wrap(text, 95) {
            self.space(LINE);
            self.doc.text(LEFT, self.y, 8.5, false, &line);
            self.y += LINE;
        }
    }

    /// Fila cuyo valor puede no caber en una línea (el veredicto, por ejemplo).
    fn row_wrapped(&mut self, label: &str, value: &str) {
        // A 8,5 pt caben unos 95 caracteres en el ancho útil; el valor empieza en LABEL_X.
        let max = ((RIGHT - LABEL_X) * 95.0 / WIDTH) as usize;
        for (index, line) in wrap(value, max.max(20)).iter().enumerate() {
            self.space(LINE);
            if index == 0 {
                self.doc.text(LEFT, self.y, 8.5, false, label);
            }
            self.doc.text(LABEL_X, self.y, 8.5, false, line);
            self.y += LINE;
        }
    }
}

/// Parte un texto en líneas de como mucho `max` caracteres, sin cortar palabras.
fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut current = String::new();
        for word in raw.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
            } else if current.chars().count() + 1 + word.chars().count() <= max {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
            }
        }
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Genera el PDF del informe.
pub fn render_pdf(data: &ReportData) -> Vec<u8> {
    let mut l = Layout::new(
        &format!("Informe {}", data.sample_code),
        &format!("Informe · muestra {}", data.sample_code),
    );

    // ---------------------------------------------------------------- cabecera
    l.doc.text(LEFT, 22.0, 16.0, true, &data.laboratory.name);
    let mut meta = Vec::new();
    if let Some(address) = &data.laboratory.address {
        meta.push(address.clone());
    }
    if let Some(accreditation) = &data.laboratory.accreditation {
        meta.push(format!("Acreditación {accreditation}"));
    }
    if !meta.is_empty() {
        l.doc.text(LEFT, 27.0, 8.0, false, &meta.join(" · "));
    }
    l.doc.line(LEFT, 31.0, RIGHT, 31.0);
    l.doc.text(LEFT, 39.0, 12.5, true, "INFORME DE ENSAYO");
    l.y = 48.0;

    // ------------------------------------------------------------- 1. muestra
    l.heading("1. Identificación de la muestra");
    l.row("Muestra", &data.sample_code);
    l.row("Cliente", &data.client);
    l.row("Proyecto", &data.project);
    l.row("Recepción", &data.received_on.to_iso());

    // --------------------------------------------------------------- 2. ensayo
    l.heading("2. Ensayo");
    l.row("Ensayo", &data.test_name);
    l.row("Método", &data.method_label);
    l.row(
        "Norma",
        data.norm_citation
            .as_deref()
            .unwrap_or("sin referencia normativa asociada"),
    );
    l.row("Fecha del ensayo", &data.performed_on.to_iso());
    l.row("Operador", &data.operator);
    if let Some(equipment) = &data.equipment {
        l.row("Equipo", equipment);
    }

    // ----------------------------------------------------- 3. datos originales
    l.heading("3. Datos originales");
    if data.measurements.is_empty() {
        l.paragraph("(sin mediciones registradas)");
    } else {
        for measurement in &data.measurements {
            let value = match &measurement.unit {
                Some(unit) => format!("{} {}", measurement.value, unit),
                None => measurement.value.clone(),
            };
            l.row(&measurement.label, &value);
        }
    }

    // ------------------------------------------------------------- 4. cálculos
    l.heading("4. Cálculos");
    if data.calculations.is_empty() {
        l.paragraph("(sin cálculos derivados)");
    } else {
        for calculation in &data.calculations {
            let value = match &calculation.unit {
                Some(unit) => format!("{} {}", calculation.value, unit),
                None => calculation.value.clone(),
            };
            l.row(&calculation.label, &value);
        }
    }

    // ------------------------------------------------------------ 5. resultado
    l.heading("5. Resultado");
    l.space(14.0);
    l.doc.band(LEFT, l.y - 4.0, WIDTH, 11.0);
    let result = match &data.result.unit {
        Some(unit) => format!("{}: {} {}", data.result.label, data.result.value, unit),
        None => format!("{}: {}", data.result.label, data.result.value),
    };
    l.doc.text(LEFT + 2.0, l.y + 1.0, 10.5, true, &result);
    l.y += 13.0;

    l.row_wrapped("Conformidad", &verdict_text(&data.conformity));

    // -------------------------------------------------------- 6. observaciones
    l.heading("6. Observaciones");
    if data.observations.trim().is_empty() {
        l.paragraph("(sin observaciones)");
    } else {
        l.paragraph(&data.observations);
    }

    // ------------------------------------------------------------------- firma
    l.space(18.0);
    l.y += 6.0;
    l.doc.line(LEFT, l.y, LEFT + 70.0, l.y);
    l.y += 5.0;
    l.doc.text(
        LEFT,
        l.y,
        8.5,
        false,
        &format!("Responsable técnico: {}", data.responsible),
    );
    l.y += LINE;
    l.doc.text(
        LEFT,
        l.y,
        8.5,
        false,
        &format!("Emitido: {}", data.issued_on),
    );

    l.footer();
    l.doc.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LaboratoryInfo, ReportLine};
    use mamut_core::CivilDate;
    use mamut_standards::{Conformity, NormBody, NormRef, NormStatus};

    fn date() -> CivilDate {
        CivilDate::new(2026, 11, 21).unwrap()
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

    fn report(measurements: usize) -> ReportData {
        ReportData {
            laboratory: LaboratoryInfo::new("Laboratorio Central").unwrap(),
            sample_code: "H-2026-0001".into(),
            client: "Constructora Andina".into(),
            project: "Torre Norte".into(),
            received_on: date(),
            test_name: "Compresión de cilindros".into(),
            method_label: "Normativo".into(),
            norm_citation: Some("ASTM C39/C39M-26".into()),
            performed_on: date(),
            operator: "Ana Torres".into(),
            equipment: None,
            measurements: (0..measurements)
                .map(|i| ReportLine::with_unit(format!("Medida {i}"), "1.0", "mm"))
                .collect(),
            calculations: vec![ReportLine::with_unit("Área", "17671.46", "mm²")],
            result: ReportLine::with_unit("Resistencia", "28.29", "MPa"),
            conformity: Conformity::evaluate(28.29, &pending_norm(), None, None),
            observations: String::new(),
            issued_on: date(),
            responsible: "Ing. Ana Torres".into(),
        }
    }

    fn text_of(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn el_informe_es_un_pdf_con_las_secciones() {
        let bytes = render_pdf(&report(3));
        let text = text_of(&bytes);
        assert!(bytes.starts_with(b"%PDF-1.4"));
        // Se buscan fragmentos sin acentos: el flujo es WinAnsi, no UTF-8.
        for expected in [
            "INFORME DE ENSAYO",
            "la muestra",
            "Datos originales",
            "Resultado",
            "Responsable",
        ] {
            assert!(text.contains(expected), "falta «{expected}» en el PDF");
        }
    }

    #[test]
    fn el_pdf_lleva_la_norma_y_el_resultado() {
        let text = text_of(&render_pdf(&report(1)));
        assert!(text.contains("ASTM C39/C39M-26"));
        assert!(text.contains("28.29"));
        assert!(text.contains("MPa"));
    }

    #[test]
    fn sin_juicio_el_informe_no_afirma_conformidad() {
        let text = text_of(&render_pdf(&report(1)));
        assert!(text.contains("SIN JUICIO DE CONFORMIDAD"));
        assert!(!text.contains("CONFORME —"), "no puede haber veredicto");
    }

    #[test]
    fn muchas_mediciones_paginan_en_varias_hojas() {
        let bytes = render_pdf(&report(80));
        let text = text_of(&bytes);
        let pages = text.matches("/Type /Page ").count();
        assert!(
            pages >= 2,
            "con 80 líneas debe haber más de una página, hay {pages}"
        );
        assert!(text.trim_end().ends_with("%%EOF"));
    }

    #[test]
    fn el_texto_largo_se_parte_en_lineas() {
        let lines = wrap(
            "una frase bastante larga que no cabe en una sola línea de informe técnico",
            20,
        );
        assert!(lines.len() > 2);
        assert!(lines.iter().all(|line| line.chars().count() <= 20));
    }
}
