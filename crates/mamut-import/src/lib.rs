//! `mamut-import` — importación de datos desde CSV.
//!
//! **Filosofía:** importar no es adivinar. Una fila que no se entiende **no entra**, y
//! el motivo se informa con su número de línea. Importar «lo que se pueda» y callar el
//! resto es la forma más rápida de meter basura en un laboratorio.
//!
//! Se lee por **nombre de columna**, no por posición: así una hoja de cálculo con
//! columnas extra (o en otro orden) sigue funcionando.

use std::collections::HashSet;

use mamut_core::CivilDate;
use mamut_domain::{Material, SampleCode};

/// Columnas obligatorias del CSV de muestras.
pub const REQUIRED_COLUMNS: [&str; 7] = [
    "codigo",
    "material",
    "procedencia",
    "cliente",
    "proyecto",
    "fecha_recepcion",
    "responsable",
];

/// Cabecera que se ofrece al usuario como plantilla.
pub const TEMPLATE_HEADER: &str =
    "codigo,material,procedencia,cliente,proyecto,fecha_recepcion,responsable";

/// Una fila válida, lista para convertirse en muestra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleDraft {
    pub code: String,
    pub material: String,
    pub origin: String,
    pub client: String,
    pub project: String,
    pub received_on: String,
    pub responsible: String,
}

/// Un problema en una fila concreta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportIssue {
    /// Número de línea del archivo (la cabecera es la 1).
    pub line: usize,
    pub code: &'static str,
    pub message: String,
}

/// Resultado de la importación.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub accepted: Vec<SampleDraft>,
    pub rejected: Vec<ImportIssue>,
}

impl ImportReport {
    pub fn is_clean(&self) -> bool {
        self.rejected.is_empty()
    }

    pub fn total_rows(&self) -> usize {
        self.accepted.len() + self.rejected.len()
    }

    pub fn has_code(&self, code: &str) -> bool {
        self.rejected.iter().any(|i| i.code == code)
    }
}

/// Parsea un CSV de muestras.
pub fn parse_samples_csv(input: &str) -> ImportReport {
    let mut report = ImportReport::default();

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(input.as_bytes());

    let headers = match reader.headers() {
        Ok(headers) => headers.clone(),
        Err(error) => {
            report.rejected.push(ImportIssue {
                line: 1,
                code: "E-CABECERA",
                message: format!("no se pudo leer la cabecera: {error}"),
            });
            return report;
        }
    };

    // Índice por nombre de columna, tolerando mayúsculas y espacios.
    let mut index = std::collections::HashMap::new();
    for (position, name) in headers.iter().enumerate() {
        index.insert(name.trim().to_ascii_lowercase(), position);
    }

    let missing: Vec<&str> = REQUIRED_COLUMNS
        .iter()
        .copied()
        .filter(|column| !index.contains_key(*column))
        .collect();
    if !missing.is_empty() {
        report.rejected.push(ImportIssue {
            line: 1,
            code: "E-COLUMNAS",
            message: format!("faltan columnas obligatorias: {}", missing.join(", ")),
        });
        return report;
    }

    let field = |record: &csv::StringRecord, column: &str| -> String {
        index
            .get(column)
            .and_then(|position| record.get(*position))
            .unwrap_or("")
            .trim()
            .to_owned()
    };

    let mut seen_codes: HashSet<String> = HashSet::new();

    for (offset, record) in reader.records().enumerate() {
        let line = offset + 2; // la cabecera ocupa la línea 1
        let record = match record {
            Ok(record) => record,
            Err(error) => {
                report.rejected.push(ImportIssue {
                    line,
                    code: "E-FILA",
                    message: format!("fila ilegible: {error}"),
                });
                continue;
            }
        };

        let code = field(&record, "codigo");
        let material_text = field(&record, "material");
        let origin = field(&record, "procedencia");
        let client = field(&record, "cliente");
        let project = field(&record, "proyecto");
        let received_on = field(&record, "fecha_recepcion");
        let responsible = field(&record, "responsable");

        if code.is_empty() {
            report.rejected.push(ImportIssue {
                line,
                code: "E-CODIGO-VACIO",
                message: "la fila no tiene código de muestra".to_owned(),
            });
            continue;
        }

        if SampleCode::new(&code).is_err() {
            report.rejected.push(ImportIssue {
                line,
                code: "E-CODIGO-INVALIDO",
                message: format!("código no válido: «{code}»"),
            });
            continue;
        }

        let normalized = code.trim().to_ascii_uppercase();
        if !seen_codes.insert(normalized.clone()) {
            report.rejected.push(ImportIssue {
                line,
                code: "E-CODIGO-DUPLICADO",
                message: format!("el código «{normalized}» aparece más de una vez en el archivo"),
            });
            continue;
        }

        if Material::parse(&material_text.to_ascii_lowercase()).is_none() {
            report.rejected.push(ImportIssue {
                line,
                code: "E-MATERIAL",
                message: format!(
                    "material desconocido: «{material_text}» (se admite concrete, soil, aggregate, asphalt)"
                ),
            });
            continue;
        }

        if CivilDate::parse(&received_on).is_err() {
            report.rejected.push(ImportIssue {
                line,
                code: "E-FECHA",
                message: format!("fecha inválida: «{received_on}» (formato AAAA-MM-DD)"),
            });
            continue;
        }

        if origin.is_empty() || client.is_empty() || project.is_empty() || responsible.is_empty() {
            report.rejected.push(ImportIssue {
                line,
                code: "E-VACIO",
                message: "procedencia, cliente, proyecto y responsable son obligatorios".to_owned(),
            });
            continue;
        }

        report.accepted.push(SampleDraft {
            code: normalized,
            material: material_text.to_ascii_lowercase(),
            origin,
            client,
            project,
            received_on,
            responsible,
        });
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv(rows: &str) -> String {
        format!("{TEMPLATE_HEADER}\n{rows}")
    }

    #[test]
    fn dos_filas_validas_entran() {
        let report = parse_samples_csv(&csv(
            "H-1,concrete,Bloque 3,Andina,Torre Norte,2026-11-18,Ana Torres\n\
             H-2,soil,Calicata 1,Andina,Torre Norte,2026-11-19,Luis Pérez",
        ));
        assert!(report.is_clean(), "{:?}", report.rejected);
        assert_eq!(report.accepted.len(), 2);
        assert_eq!(report.accepted[0].code, "H-1");
        assert_eq!(report.accepted[1].material, "soil");
    }

    #[test]
    fn los_codigos_se_normalizan_a_mayusculas() {
        let report = parse_samples_csv(&csv("h-1,concrete,Bloque 3,Andina,Torre,2026-11-18,Ana"));
        assert_eq!(report.accepted[0].code, "H-1");
    }

    #[test]
    fn faltar_una_columna_impide_importar() {
        let report = parse_samples_csv("codigo,material\nH-1,concrete\n");
        assert!(report.has_code("E-COLUMNAS"));
        assert!(report.accepted.is_empty(), "no se importa nada a medias");
    }

    #[test]
    fn una_fecha_invalida_rechaza_la_fila_y_no_el_archivo() {
        let report = parse_samples_csv(&csv("H-1,concrete,Bloque 3,Andina,Torre,2026-02-31,Ana"));
        assert!(report.has_code("E-FECHA"));
        assert_eq!(report.accepted.len(), 0);
        assert_eq!(report.rejected.len(), 1);
    }

    #[test]
    fn un_material_desconocido_se_rechaza() {
        let report = parse_samples_csv(&csv("H-1,madera,Bloque 3,Andina,Torre,2026-11-18,Ana"));
        assert!(report.has_code("E-MATERIAL"));
    }

    #[test]
    fn un_codigo_repetido_en_el_archivo_se_rechaza() {
        let report = parse_samples_csv(&csv("H-1,concrete,Bloque 3,Andina,Torre,2026-11-18,Ana\n\
             h-1,concrete,Bloque 4,Andina,Torre,2026-11-19,Ana"));
        assert!(report.has_code("E-CODIGO-DUPLICADO"));
        assert_eq!(report.accepted.len(), 1);
    }

    #[test]
    fn una_fila_sin_codigo_se_rechaza() {
        let report = parse_samples_csv(&csv(",concrete,Bloque 3,Andina,Torre,2026-11-18,Ana"));
        assert!(report.has_code("E-CODIGO-VACIO"));
    }

    #[test]
    fn un_campo_obligatorio_vacio_se_rechaza() {
        let report = parse_samples_csv(&csv("H-1,concrete,,Andina,Torre,2026-11-18,Ana"));
        assert!(report.has_code("E-VACIO"));
    }

    #[test]
    fn las_columnas_extra_y_el_orden_distinto_no_molestan() {
        let report = parse_samples_csv(
            "cliente,codigo,material,procedencia,proyecto,fecha_recepcion,responsable,estado\n\
             Andina,H-1,concrete,Bloque 3,Torre,2026-11-18,Ana,recibida\n",
        );
        assert!(report.is_clean(), "{:?}", report.rejected);
        assert_eq!(report.accepted[0].code, "H-1");
        assert_eq!(report.accepted[0].client, "Andina");
    }

    #[test]
    fn el_error_dice_en_que_linea_esta() {
        let report = parse_samples_csv(&csv("H-1,concrete,Bloque 3,Andina,Torre,2026-11-18,Ana\n\
             H-2,concrete,Bloque 3,Andina,Torre,no-es-fecha,Ana"));
        assert_eq!(report.rejected[0].line, 3, "la cabecera es la línea 1");
    }
}
