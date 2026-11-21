//! Probe: XLSX export.
//!
//! Only proves the export crate builds and writes a valid workbook locally.
//! The real exporters (`lab-export`) arrive in a later phase.

use std::path::Path;

use rust_xlsxwriter::{Workbook, XlsxError};

pub fn write_xlsx(path: &Path, rows: &[(&str, f64)]) -> Result<(), XlsxError> {
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("Resultados")?;
    sheet.write_string(0, 0, "Muestra")?;
    sheet.write_string(0, 1, "σ (MPa)")?;
    for (i, (id, strength)) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        sheet.write_string(r, 0, *id)?;
        sheet.write_number(r, 1, *strength)?;
    }
    book.save(path)?;
    Ok(())
}
