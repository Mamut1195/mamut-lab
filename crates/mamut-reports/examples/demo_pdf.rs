#![allow(clippy::expect_used)]
//! Genera un informe de ejemplo para revisión visual.
//!
//! ```sh
//! cargo run -p mamut-reports --example demo_pdf -- informe.pdf
//! ```
//!
//! Los datos son **sintéticos**: sirve para comprobar la maquetación, no como resultado.

use mamut_core::CivilDate;
use mamut_reports::{LaboratoryInfo, ReportData, ReportLine, render_pdf};
use mamut_standards::{Conformity, NormBody, NormRef, NormStatus};

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "informe-demo.pdf".to_owned());

    let date = CivilDate::new(2026, 11, 21).expect("fecha válida");
    let norm = NormRef::new(
        NormBody::Astm,
        "C39/C39M",
        2026,
        "Compresión de cilindros",
        NormStatus::PendingReview,
    )
    .expect("norma válida");

    let data = ReportData {
        laboratory: LaboratoryInfo::new("Laboratorio Central de Materiales").expect("nombre"),
        sample_code: "H-2026-0001".into(),
        client: "Constructora Andina".into(),
        project: "Torre Norte".into(),
        received_on: CivilDate::new(2026, 11, 1).expect("fecha"),
        test_name: "Compresión de cilindros de hormigón".into(),
        method_label: "Normativo".into(),
        norm_citation: Some(norm.citation()),
        performed_on: CivilDate::new(2026, 11, 29).expect("fecha"),
        operator: "Ana Torres".into(),
        equipment: Some("Máquina universal 1".into()),
        measurements: vec![
            ReportLine::with_unit("Diámetro", "150.0", "mm"),
            ReportLine::with_unit("Altura", "300.0", "mm"),
            ReportLine::with_unit("Masa", "12300.0", "g"),
            ReportLine::with_unit("Carga máxima", "500.0", "kN"),
        ],
        calculations: vec![
            ReportLine::with_unit("Área de sección", "17671.46", "mm²"),
            ReportLine::with_unit("Relación altura/diámetro", "2.00", ""),
        ],
        result: ReportLine::with_unit("Resistencia a compresión", "28.29", "MPa"),
        conformity: Conformity::evaluate(28.29, &norm, None, None),
        observations: "Espécimen refrentado con pasta de cemento. Sin incidencias durante el ensayo. \
                       Este documento es una demostración de maquetación y no un resultado de laboratorio."
            .into(),
        issued_on: date,
        responsible: "Ing. Ana Torres".into(),
    };

    let bytes = render_pdf(&data);
    std::fs::write(&path, &bytes).expect("escribir el PDF");
    println!("escrito {path} ({} bytes)", bytes.len());
}
