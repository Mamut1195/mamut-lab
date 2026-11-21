//! CBR — California Bearing Ratio.
//!
//! **Referencia pública:** Ohio DOT, *Procedure Checklist T-193 (ASTM D1883)*, mayo 2024 —
//! confirma que el CBR se obtiene dividiendo la carga unitaria corregida a 2,54 mm y 5,08 mm
//! entre las cargas estándar de **6,9 y 10,3 MPa (1000 y 1500 psi)** y multiplicando por 100.
//! La corrección de la curva inicial está descrita en FDOT `FM 5-515` §7.1.
//! Análisis en `docs/normas/referencias-publicas/cbr-lbr.md`.
//!
//! **Por qué no se copió el LBR.** El método de Florida divide entre 800 psi (referencia de
//! la caliza de Florida). Usar ese divisor y llamarlo CBR daría valores más altos en todos
//! los suelos: es un error de ingeniería, no una preferencia.

use crate::{CalcError, Result};

/// 1000 psi en MPa. Se deriva del valor en psi en lugar de redondearlo: usar 10,3 en vez de
/// 10,3421 desplazaba el resultado del 0,20 in un 0,41 %, suficiente para cambiar un informe
/// redondeado a entero.
pub const STANDARD_LOAD_01_MPA: f64 = 6.894_757;

/// 1500 psi en MPa.
pub const STANDARD_LOAD_02_MPA: f64 = 10.342_136;

/// CBR por encima de este valor no es creíble y casi siempre es un error de unidades.
pub const MAX_PLAUSIBLE_CBR: f64 = 200.0;

/// Un punto de la curva carga–penetración.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenetrationPoint {
    pub penetration_mm: f64,
    /// Carga unitaria (carga dividida por el área del pistón), en MPa.
    pub load_mpa: f64,
}

/// Resultado del ensayo.
///
/// Se informa **qué penetración gobernó** y **cuánto se corrigió el origen**: son datos del
/// ensayo, no adornos. Dos suelos con el mismo CBR pueden haberlo alcanzado por penetraciones
/// distintas, y un CBR corregido no se puede comparar con uno que no lo fue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CbrResult {
    pub percent: f64,
    /// Penetración que gobernó el resultado: 2,54 o 5,08 mm.
    pub governing_penetration_mm: f64,
    /// Corrimiento del origen aplicado por la corrección de la curva, en mm.
    pub offset_mm: f64,
}

fn check_load(value: f64, penetration_mm: f64) -> Result<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(CalcError::Incoherent(format!(
            "la carga a {penetration_mm} mm debe ser mayor que cero (es {value})"
        )));
    }
    Ok(value)
}

fn check_plausible(percent: f64) -> Result<()> {
    if percent > MAX_PLAUSIBLE_CBR {
        return Err(CalcError::Incoherent(format!(
            "un CBR de {percent:.0} % no es creíble: revisa las unidades de la carga \
             (¿MPa o psi?)"
        )));
    }
    Ok(())
}

/// CBR a partir de las dos cargas **ya corregidas**.
///
/// Se calcula el porcentaje en las dos penetraciones y se informa **el mayor**, que es lo que
/// indica el método: un suelo puede dar más resistencia relativa a 5,08 mm que a 2,54 mm.
pub fn cbr_percent(load_01_mpa: f64, load_02_mpa: f64) -> Result<CbrResult> {
    let first_load = check_load(load_01_mpa, 2.54)?;
    let second_load = check_load(load_02_mpa, 5.08)?;

    let first = 100.0 * first_load / STANDARD_LOAD_01_MPA;
    let second = 100.0 * second_load / STANDARD_LOAD_02_MPA;
    check_plausible(first)?;
    check_plausible(second)?;

    let (percent, governing) = if first >= second {
        (first, 2.54)
    } else {
        (second, 5.08)
    };

    Ok(CbrResult {
        percent,
        governing_penetration_mm: governing,
        offset_mm: 0.0,
    })
}

/// Desplazamiento del origen por la concavidad inicial de la curva.
///
/// Si el tramo inicial es cóncavo hacia arriba (asiento de la placa, irregularidades de la
/// superficie), se traza la tangente en el punto de máxima pendiente y el origen se mueve a
/// donde esa tangente corta el eje de penetración.
///
/// Devuelve `None` cuando la curva ya es convexa desde el origen o cuando el corrimiento
/// resultante no tiene sentido físico (mayor que 1 mm o que el 20 % de la penetración de
/// referencia): en ese caso lo que está mal es la curva, no el origen.
pub fn tangent_origin_offset(points: &[PenetrationPoint]) -> Option<f64> {
    if points.len() < 3 {
        return None;
    }
    let mut ordered = points.to_vec();
    ordered.sort_by(|a, b| a.penetration_mm.total_cmp(&b.penetration_mm));

    let mut best = (f64::NEG_INFINITY, 0usize);
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
    if slope <= 0.0 || !slope.is_finite() {
        return None;
    }

    let x1 = ordered[index + 1].penetration_mm;
    let y1 = ordered[index + 1].load_mpa;
    let intercept = y1 - slope * x1;
    let crossing = -intercept / slope;

    if !crossing.is_finite() || crossing <= 0.0 {
        return None;
    }
    // Cota de sentido físico.
    if crossing > 1.0 || crossing > 0.2 * 5.08 {
        return None;
    }
    Some(crossing)
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
pub fn cbr_from_curve(points: &[PenetrationPoint]) -> Result<CbrResult> {
    if points.len() < 3 {
        return Err(CalcError::Incoherent(
            "la curva necesita al menos tres puntos de penetración".into(),
        ));
    }
    let mut ordered = points.to_vec();
    ordered.sort_by(|a, b| a.penetration_mm.total_cmp(&b.penetration_mm));

    let offset = tangent_origin_offset(&ordered).unwrap_or(0.0);

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

    let mut result = cbr_percent(first, second)?;
    result.offset_mm = offset;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn media_carga_estandar_da_cbr_cincuenta_con_el_divisor_real() {
        // Valores fijados a mano, no derivados de las constantes: si alguien cambiara el
        // divisor por 800 psi (LBR), esta prueba fallaría.
        // 3,4474 MPa ≈ 500 psi → 50 % de 1000 psi. 5,1711 MPa ≈ 750 psi → 50 % de 1500 psi.
        let result = cbr_percent(3.447_4, 5.171_1).unwrap();
        assert!(
            close(result.percent, 50.0, 0.01),
            "salió {}",
            result.percent
        );
        assert_eq!(result.governing_penetration_mm, 2.54);
    }

    #[test]
    fn se_informa_el_mayor_de_los_dos_y_cual_goberno() {
        // A 2,54 mm da 50 %; a 5,08 mm da 80 %. Se informa 80 y que gobernó el de 5,08.
        let result = cbr_percent(3.45, 8.27).unwrap();
        assert!(close(result.percent, 80.0, 0.1), "salió {}", result.percent);
        assert_eq!(result.governing_penetration_mm, 5.08);
    }

    #[test]
    fn una_carga_nula_o_negativa_no_es_un_ensayo() {
        assert!(cbr_percent(0.0, 5.0).is_err());
        assert!(cbr_percent(-1.0, 5.0).is_err());
        assert!(cbr_percent(1.0, f64::NAN).is_err());
        assert!(cbr_percent(1.0, 0.0).is_err());
    }

    #[test]
    fn un_cbr_imposible_avisa_de_unidades() {
        // 1000 y 1500 MPa son psi tecleados como MPa.
        let error = cbr_percent(1000.0, 1500.0).unwrap_err();
        assert!(error.to_string().contains("unidades"), "{error}");
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
                load_mpa: 3.447_4,
            },
            PenetrationPoint {
                penetration_mm: 5.08,
                load_mpa: 5.171_1,
            },
        ];
        assert!(tangent_origin_offset(&points).is_none());
        let result = cbr_from_curve(&points).unwrap();
        assert!(close(result.percent, 50.0, 0.01));
        assert_eq!(result.offset_mm, 0.0);
    }

    #[test]
    fn una_curva_concava_desplaza_el_origen_y_lo_informa() {
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
            // La curva debe llegar más allá de 5,08 mm: con el origen corregido, la lectura
            // del 0,20 in se busca por encima de esa penetración.
            PenetrationPoint {
                penetration_mm: 6.50,
                load_mpa: 1.35,
            },
        ];
        let offset = tangent_origin_offset(&points).expect("hay corrección");
        assert!(offset > 0.0);
        assert!(offset <= 1.0, "el corrimiento está acotado: {offset}");

        let result = cbr_from_curve(&points).unwrap();
        assert!(close(result.offset_mm, offset, 1e-12));
        assert!(result.percent > 0.0);
    }

    #[test]
    fn una_curva_que_se_empina_tarde_no_se_corrige() {
        // El tramo de máxima pendiente está al final: un corrimiento de varios milímetros no
        // tiene sentido físico.
        let points = [
            PenetrationPoint {
                penetration_mm: 0.0,
                load_mpa: 0.0,
            },
            PenetrationPoint {
                penetration_mm: 0.5,
                load_mpa: 0.30,
            },
            PenetrationPoint {
                penetration_mm: 1.0,
                load_mpa: 0.60,
            },
            PenetrationPoint {
                penetration_mm: 2.54,
                load_mpa: 1.50,
            },
            PenetrationPoint {
                penetration_mm: 4.0,
                load_mpa: 2.00,
            },
            PenetrationPoint {
                penetration_mm: 5.08,
                load_mpa: 4.50,
            },
        ];
        assert!(tangent_origin_offset(&points).is_none());
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
