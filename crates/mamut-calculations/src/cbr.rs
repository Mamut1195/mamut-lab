//! CBR — California Bearing Ratio.
//!
//! **Referencia pública:** Ohio DOT, *Procedure Checklist T-193 (ASTM D1883)*, mayo 2024 —
//! confirma que el CBR se obtiene dividiendo la carga unitaria corregida a 2,54 mm y 5,08 mm
//! entre las cargas estándar de **6,9 y 10,3 MPa (1000 y 1500 psi)** y multiplicando por 100.
//! La corrección de la curva inicial está descrita en FDOT `FM 5-515` §7.1 (que es LBR, pero
//! comparte el procedimiento de corrección).
//! Análisis en `docs/normas/referencias-publicas/cbr-lbr.md`.
//!
//! **Por qué no se copió el LBR.** El método de Florida divide entre 800 psi (referencia de
//! la caliza de Florida). Usar ese divisor y llamarlo CBR daría valores más altos en todos
//! los suelos: es un error de ingeniería, no una preferencia.

use crate::{CalcError, Result};

/// Carga unitaria estándar a 2,54 mm (0,10 in) de penetración, en MPa.
pub const STANDARD_LOAD_01_MPA: f64 = 6.9;

/// Carga unitaria estándar a 5,08 mm (0,20 in) de penetración, en MPa.
pub const STANDARD_LOAD_02_MPA: f64 = 10.3;

/// Un punto de la curva carga–penetración.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenetrationPoint {
    pub penetration_mm: f64,
    /// Carga unitaria (carga dividida por el área del pistón), en MPa.
    pub load_mpa: f64,
}

/// CBR a partir de las dos cargas corregidas.
///
/// Se calcula el porcentaje en las dos penetraciones y se informa **el mayor**, que es lo que
/// indica el método: un suelo puede dar más resistencia relativa a 5,08 mm que a 2,54 mm.
pub fn cbr_percent(load_01_mpa: f64, load_02_mpa: f64) -> Result<f64> {
    if !load_01_mpa.is_finite() || load_01_mpa < 0.0 {
        return Err(CalcError::Incoherent(format!(
            "carga a 2,54 mm no válida: {load_01_mpa} MPa"
        )));
    }
    if !load_02_mpa.is_finite() || load_02_mpa < 0.0 {
        return Err(CalcError::Incoherent(format!(
            "carga a 5,08 mm no válida: {load_02_mpa} MPa"
        )));
    }
    let first = 100.0 * load_01_mpa / STANDARD_LOAD_01_MPA;
    let second = 100.0 * load_02_mpa / STANDARD_LOAD_02_MPA;
    Ok(first.max(second))
}

/// Desplazamiento del origen por la concavidad inicial de la curva.
///
/// Si el tramo inicial es cóncavo hacia arriba (asiento de la placa, irregularidades de la
/// superficie), se traza la tangente en el punto de máxima pendiente y el origen se mueve a
/// donde esa tangente corta el eje de penetración. Devuelve ese corrimiento en milímetros;
/// `0` cuando la curva ya es convexa desde el origen.
pub fn tangent_origin_offset(points: &[PenetrationPoint]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut ordered = points.to_vec();
    ordered.sort_by(|a, b| a.penetration_mm.total_cmp(&b.penetration_mm));

    // Pendiente de cada tramo.
    let mut best = (0.0_f64, 0usize);
    for index in 0..ordered.len() - 1 {
        let dx = ordered[index + 1].penetration_mm - ordered[index].penetration_mm;
        let dy = ordered[index + 1].load_mpa - ordered[index].load_mpa;
        if dx <= 0.0 {
            continue;
        }
        let slope = dy / dx;
        if slope > best.0 {
            best = (slope, index);
        }
    }
    let (slope, index) = best;
    if slope <= 0.0 {
        return 0.0;
    }

    // Recta tangente por el punto final del tramo de máxima pendiente.
    let (x1, y1) = (
        ordered[index + 1].penetration_mm,
        ordered[index + 1].load_mpa,
    );
    let intercept = y1 - slope * x1;
    // Corte con el eje de penetración (carga nula).
    let crossing = -intercept / slope;

    // Un corrimiento negativo no tiene sentido físico: la curva ya pasa por el origen.
    if crossing.is_finite() && crossing > 0.0 {
        crossing
    } else {
        0.0
    }
}

/// Carga unitaria interpolada a una penetración dada.
fn load_at(points: &[PenetrationPoint], target_mm: f64) -> Option<f64> {
    for pair in points.windows(2) {
        let (x0, y0) = (pair[0].penetration_mm, pair[0].load_mpa);
        let (x1, y1) = (pair[1].penetration_mm, pair[1].load_mpa);
        if (x0..=x1).contains(&target_mm) {
            if (x1 - x0).abs() < f64::EPSILON {
                return Some(y1);
            }
            let t = (target_mm - x0) / (x1 - x0);
            return Some(y0 + t * (y1 - y0));
        }
    }
    None
}

/// CBR a partir de la curva completa, aplicando la corrección de origen.
///
/// Las cargas se leen en las penetraciones corregidas: si el origen se desplazó 0,4 mm, la
/// carga de 2,54 mm se busca en 2,94 mm de la curva medida.
pub fn cbr_from_curve(points: &[PenetrationPoint]) -> Result<f64> {
    if points.len() < 3 {
        return Err(CalcError::Incoherent(
            "la curva necesita al menos tres puntos de penetración".into(),
        ));
    }
    let offset = tangent_origin_offset(points);
    let mut ordered = points.to_vec();
    ordered.sort_by(|a, b| a.penetration_mm.total_cmp(&b.penetration_mm));

    let first = load_at(&ordered, 2.54 + offset).ok_or_else(|| {
        CalcError::Incoherent(format!(
            "la curva no llega a la penetración corregida de 2,54 mm (corrimiento {offset:.2} mm)"
        ))
    })?;
    let second = load_at(&ordered, 5.08 + offset).ok_or_else(|| {
        CalcError::Incoherent(format!(
            "la curva no llega a la penetración corregida de 5,08 mm (corrimiento {offset:.2} mm)"
        ))
    })?;

    cbr_percent(first, second)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn la_carga_estandar_da_cbr_cien() {
        // 6,9 MPa a 2,54 mm y 10,3 MPa a 5,08 mm son exactamente el 100 %.
        let cbr = cbr_percent(STANDARD_LOAD_01_MPA, STANDARD_LOAD_02_MPA).unwrap();
        assert!(close(cbr, 100.0, 1e-9));
    }

    #[test]
    fn media_carga_estandar_da_cbr_cincuenta() {
        let cbr = cbr_percent(STANDARD_LOAD_01_MPA / 2.0, STANDARD_LOAD_02_MPA / 4.0).unwrap();
        assert!(close(cbr, 50.0, 1e-9));
    }

    #[test]
    fn se_informa_el_mayor_de_los_dos_porcentajes() {
        // A 2,54 mm da 50 %; a 5,08 mm da 80 %. Se informa 80.
        let cbr = cbr_percent(3.45, 8.24).unwrap();
        assert!(close(cbr, 80.0, 1e-9), "salió {cbr}");
    }

    #[test]
    fn una_carga_negativa_se_rechaza() {
        assert!(cbr_percent(-1.0, 5.0).is_err());
        assert!(cbr_percent(1.0, f64::NAN).is_err());
    }

    #[test]
    fn una_curva_lineal_desde_el_origen_no_se_corrige() {
        let points = [
            PenetrationPoint {
                penetration_mm: 0.0,
                load_mpa: 0.0,
            },
            PenetrationPoint {
                penetration_mm: 2.54,
                load_mpa: 3.45,
            },
            PenetrationPoint {
                penetration_mm: 5.08,
                load_mpa: 5.0,
            },
        ];
        assert!(close(tangent_origin_offset(&points), 0.0, 1e-12));
        let cbr = cbr_from_curve(&points).unwrap();
        assert!(close(cbr, 50.0, 1e-6), "salió {cbr}");
    }

    #[test]
    fn una_curva_concava_desplaza_el_origen_hacia_adelante() {
        // Tramo inicial casi plano y luego subida fuerte: hay corrección.
        let points = [
            PenetrationPoint {
                penetration_mm: 0.0,
                load_mpa: 0.0,
            },
            PenetrationPoint {
                penetration_mm: 0.5,
                load_mpa: 0.02,
            },
            PenetrationPoint {
                penetration_mm: 1.0,
                load_mpa: 0.05,
            },
            PenetrationPoint {
                penetration_mm: 2.54,
                load_mpa: 0.60,
            },
            PenetrationPoint {
                penetration_mm: 5.08,
                load_mpa: 1.10,
            },
        ];
        let offset = tangent_origin_offset(&points);
        assert!(offset > 0.0, "debe haber corrimiento, salió {offset}");
        assert!(offset < 2.54, "el corrimiento no puede tragarse la curva");
    }

    #[test]
    fn una_curva_que_no_llega_a_la_penetracion_corregida_no_inventa_cbr() {
        let points = [
            PenetrationPoint {
                penetration_mm: 0.0,
                load_mpa: 0.0,
            },
            PenetrationPoint {
                penetration_mm: 1.0,
                load_mpa: 0.1,
            },
            PenetrationPoint {
                penetration_mm: 1.5,
                load_mpa: 0.2,
            },
        ];
        assert!(cbr_from_curve(&points).is_err());
    }

    #[test]
    fn hacen_falta_tres_puntos_de_curva() {
        assert!(
            cbr_from_curve(&[PenetrationPoint {
                penetration_mm: 2.54,
                load_mpa: 3.0
            }])
            .is_err()
        );
    }
}
