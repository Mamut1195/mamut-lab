//! `mamut-export` — exportación a CSV y XLSX.
//!
//! La cabecera del CSV coincide con la que espera `mamut-import`, de modo que
//! **exportar e importar es un viaje de ida y vuelta** sin pérdida, y eso se prueba.
//! Es la forma de que el usuario pueda sacar sus datos y volver a meterlos: si el
//! formato de salida no se puede leer, la exportación es un adorno.

use std::path::Path;

use rust_xlsxwriter::Workbook;

/// Cabecera del CSV de muestras. Las siete primeras columnas son las que exige la
/// importación; `estado` es informativa y se ignora al reimportar.
pub const HEADER: [&str; 8] = [
    "codigo",
    "material",
    "procedencia",
    "cliente",
    "proyecto",
    "fecha_recepcion",
    "responsable",
    "estado",
];

/// Una fila de muestra lista para exportar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleRow {
    pub code: String,
    pub material: String,
    pub origin: String,
    pub client: String,
    pub project: String,
    pub received_on: String,
    pub responsible: String,
    pub state: String,
}

impl SampleRow {
    fn cells(&self) -> [&str; 8] {
        [
            &self.code,
            &self.material,
            &self.origin,
            &self.client,
            &self.project,
            &self.received_on,
            &self.responsible,
            &self.state,
        ]
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("error de CSV: {0}")]
    Csv(#[from] csv::Error),

    #[error("error al escribir XLSX: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),

    #[error("error de entrada/salida: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ExportError>;

/// Serializa las muestras a CSV (con cabecera).
pub fn samples_to_csv(rows: &[SampleRow]) -> Result<String> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(Vec::new());

    writer.write_record(HEADER)?;
    for row in rows {
        writer.write_record(row.cells())?;
    }

    let bytes = writer.into_inner().map_err(|error| error.into_error())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Escribe un libro XLSX con una hoja de muestras.
pub fn write_samples_xlsx(path: &Path, rows: &[SampleRow]) -> Result<()> {
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("Muestras")?;

    for (column, title) in HEADER.iter().enumerate() {
        sheet.write_string(0, column as u16, *title)?;
    }
    for (index, row) in rows.iter().enumerate() {
        let line = (index + 1) as u32;
        for (column, value) in row.cells().iter().enumerate() {
            sheet.write_string(line, column as u16, *value)?;
        }
    }

    book.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(code: &str) -> SampleRow {
        SampleRow {
            code: code.to_owned(),
            material: "concrete".into(),
            origin: "Bloque 3".into(),
            client: "Andina".into(),
            project: "Torre Norte".into(),
            received_on: "2026-11-18".into(),
            responsible: "Ana Torres".into(),
            state: "received".into(),
        }
    }

    #[test]
    fn el_csv_empieza_por_la_cabecera() {
        let text = samples_to_csv(&[row("H-1")]).unwrap();
        let first = text.lines().next().unwrap();
        assert_eq!(first, HEADER.join(","));
        assert!(text.contains("H-1"));
    }

    #[test]
    fn un_campo_con_coma_se_entrecomilla() {
        let mut r = row("H-1");
        r.origin = "Bloque 3, planta 5".into();
        let text = samples_to_csv(&[r]).unwrap();
        assert!(
            text.contains("\"Bloque 3, planta 5\""),
            "el CSV debe escapar la coma: {text}"
        );
    }

    #[test]
    fn el_csv_exportado_se_reimporta_sin_perdida() {
        let rows = vec![row("H-1"), row("H-2")];
        let text = samples_to_csv(&rows).unwrap();
        let report = mamut_import::parse_samples_csv(&text);
        assert!(report.is_clean(), "{:?}", report.rejected);
        assert_eq!(report.accepted.len(), 2);
        assert_eq!(report.accepted[0].code, "H-1");
        assert_eq!(report.accepted[0].origin, "Bloque 3");
        assert_eq!(report.accepted[1].client, "Andina");
    }

    #[test]
    fn el_xlsx_es_un_archivo_real() {
        let dir = std::env::temp_dir().join(format!("mamut-xlsx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("muestras.xlsx");

        write_samples_xlsx(&path, &[row("H-1"), row("H-2")]).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 1_000, "un XLSX no pesa unos pocos bytes");
        assert_eq!(&bytes[0..2], b"PK", "XLSX es un contenedor ZIP");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
