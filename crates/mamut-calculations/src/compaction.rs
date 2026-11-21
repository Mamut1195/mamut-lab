//! Compactación: densidades, humedad y curva de compactación.
//!
//! **Referencia pública:** TxDOT `Tex-113-E` (§8.5–8.7) y `Tex-114-E`; FDOT `FM 1-T 180`.
//! Equivalencia de referencia: ASTM D698 (estándar) y D1557 (modificado); AASHTO T 99/T 180.
//! Análisis en `docs/normas/referencias-publicas/compactacion.md`.

use crate::{CalcError, Result};

/// Densidad húmeda del molde: `(masa molde+muestra − masa molde) / volumen`.
pub fn wet_density(mold_plus_soil_g: f64, mold_g: f64, volume_cm3: f64) -> Result<f64> {
    if !mold_g.is_finite() || mold_g < 0.0 {
        return Err(CalcError::Incoherent(format!(
            "masa del molde no válida: {mold_g} g"
        )));
    }
    if !volume_cm3.is_finite() || volume_cm3 <= 0.0 {
        return Err(CalcError::Incoherent(format!(
            "volumen del molde no válido: {volume_cm3} cm³"
        )));
    }
    let soil = mold_plus_soil_g - mold_g;
    if !soil.is_finite() || soil <= 0.0 {
        return Err(CalcError::Incoherent(
            "la muestra debe pesar más que el molde vacío".into(),
        ));
    }
    Ok(soil / volume_cm3)
}

/// Humedad de la muestra compactada: `100 · (húmeda − seca) / seca`.
pub fn moisture_percent(wet_g: f64, dry_g: f64) -> Result<f64> {
    if !dry_g.is_finite() || dry_g <= 0.0 {
        return Err(CalcError::Incoherent(format!(
            "masa seca no válida: {dry_g} g"
        )));
    }
    if !wet_g.is_finite() || wet_g < dry_g {
        return Err(CalcError::Incoherent(
            "la masa húmeda no puede ser menor que la seca".into(),
        ));
    }
    Ok(100.0 * (wet_g - dry_g) / dry_g)
}

/// Densidad seca: `γ_d = 100 · γ_húmeda / (100 + w)`.
pub fn dry_density(wet_density_g_cm3: f64, moisture_percent: f64) -> Result<f64> {
    if !wet_density_g_cm3.is_finite() || wet_density_g_cm3 <= 0.0 {
        return Err(CalcError::Incoherent(format!(
            "densidad húmeda no válida: {wet_density_g_cm3} g/cm³"
        )));
    }
    if !moisture_percent.is_finite() || moisture_percent < 0.0 {
        return Err(CalcError::Incoherent(format!(
            "humedad no válida: {moisture_percent} %"
        )));
    }
    Ok(100.0 * wet_density_g_cm3 / (100.0 + moisture_percent))
}

/// Un punto de la curva de compactación.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompactionPoint {
    pub moisture_percent: f64,
    pub dry_density_g_cm3: f64,
}

/// Máximo de la curva: humedad óptima y densidad seca máxima.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvePeak {
    pub optimum_moisture_percent: f64,
    pub max_dry_density_g_cm3: f64,
    /// Puntos que sostienen el máximo. Con tres es el mínimo para ajustar una parábola.
    pub points_used: usize,
}

/// Busca el máximo de la curva ajustando una parábola a los tres puntos que rodean al
/// mayor valor medido.
///
/// Devuelve `None` cuando no hay un máximo interior: con menos de tres puntos, cuando el
/// mayor está en un extremo (la curva seguiría subiendo o bajando) o cuando la parábola
/// abre hacia arriba. Es preferible no dar un número a inventarlo.
pub fn curve_peak(points: &[CompactionPoint]) -> Option<CurvePeak> {
    if points.len() < 3 {
        return None;
    }
    let mut ordered = points.to_vec();
    ordered.sort_by(|a, b| a.moisture_percent.total_cmp(&b.moisture_percent));

    let mut best = 0usize;
    for (index, point) in ordered.iter().enumerate() {
        if point.dry_density_g_cm3 > ordered[best].dry_density_g_cm3 {
            best = index;
        }
    }
    // El máximo tiene que estar entre puntos, no en un extremo.
    if best == 0 || best + 1 >= ordered.len() {
        return None;
    }

    let (x0, y0) = (
        ordered[best - 1].moisture_percent,
        ordered[best - 1].dry_density_g_cm3,
    );
    let (x1, y1) = (
        ordered[best].moisture_percent,
        ordered[best].dry_density_g_cm3,
    );
    let (x2, y2) = (
        ordered[best + 1].moisture_percent,
        ordered[best + 1].dry_density_g_cm3,
    );

    let denominator = (x0 - x1) * (x0 - x2) * (x1 - x2);
    if denominator.abs() < f64::EPSILON {
        return None;
    }

    // Coeficientes del polinomio que pasa por los tres puntos.
    let a = (x2 * (y1 - y0) + x1 * (y0 - y2) + x0 * (y2 - y1)) / denominator;
    let b = (x2 * x2 * (y0 - y1) + x1 * x1 * (y2 - y0) + x0 * x0 * (y1 - y2)) / denominator;
    let c = (x1 * x2 * (x1 - x2) * y0 + x2 * x0 * (x2 - x0) * y1 + x0 * x1 * (x0 - x1) * y2)
        / denominator;

    if a >= 0.0 || !a.is_finite() {
        return None;
    }

    let vertex_x = -b / (2.0 * a);
    let vertex_y = c - b * b / (4.0 * a);
    if !vertex_x.is_finite() || !vertex_y.is_finite() {
        return None;
    }

    Some(CurvePeak {
        optimum_moisture_percent: vertex_x,
        max_dry_density_g_cm3: vertex_y,
        points_used: 3,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn densidad_humeda_del_molde() {
        // 5000 g con molde, 3000 g de molde, 943 cm³ → 2,1209 g/cm³.
        let density = wet_density(5000.0, 3000.0, 943.0).unwrap();
        assert!(close(density, 2.120_891, 1e-6));
    }

    #[test]
    fn un_molde_mas_pesado_que_el_conjunto_es_incoherente() {
        assert!(wet_density(3000.0, 5000.0, 943.0).is_err());
        assert!(wet_density(5000.0, 3000.0, 0.0).is_err());
    }

    #[test]
    fn humedad_de_la_muestra_compactada() {
        // 220 g húmedos, 200 g secos → 10 %.
        assert!(close(moisture_percent(220.0, 200.0).unwrap(), 10.0, 1e-12));
    }

    #[test]
    fn densidad_seca_desde_la_humeda() {
        // 2,1209 g/cm³ al 10 % → 1,9281 g/cm³.
        let dry = dry_density(2.120_891, 10.0).unwrap();
        assert!(close(dry, 1.928_083, 1e-6));
    }

    #[test]
    fn la_densidad_seca_baja_al_subir_la_humedad() {
        let seco = dry_density(2.0, 5.0).unwrap();
        let humedo = dry_density(2.0, 20.0).unwrap();
        assert!(seco > humedo);
    }

    #[test]
    fn el_maximo_de_la_curva_sale_de_los_tres_puntos_que_lo_rodean() {
        // Parábola con vértice en 10,33 % y 1,952 g/cm³.
        let peak = curve_peak(&[
            CompactionPoint {
                moisture_percent: 8.0,
                dry_density_g_cm3: 1.85,
            },
            CompactionPoint {
                moisture_percent: 10.0,
                dry_density_g_cm3: 1.95,
            },
            CompactionPoint {
                moisture_percent: 12.0,
                dry_density_g_cm3: 1.90,
            },
        ])
        .unwrap();
        assert!(close(peak.optimum_moisture_percent, 10.333, 0.01));
        assert!(close(peak.max_dry_density_g_cm3, 1.952, 0.01));
        assert_eq!(peak.points_used, 3);
    }

    #[test]
    fn sin_tres_puntos_no_hay_maximo() {
        assert!(
            curve_peak(&[CompactionPoint {
                moisture_percent: 10.0,
                dry_density_g_cm3: 1.9
            }])
            .is_none()
        );
        assert!(
            curve_peak(&[
                CompactionPoint {
                    moisture_percent: 8.0,
                    dry_density_g_cm3: 1.85
                },
                CompactionPoint {
                    moisture_percent: 10.0,
                    dry_density_g_cm3: 1.95
                },
            ])
            .is_none()
        );
    }

    #[test]
    fn un_maximo_en_el_extremo_no_es_un_maximo() {
        // La densidad crece con la humedad: el máximo medido está en el último punto.
        assert!(
            curve_peak(&[
                CompactionPoint {
                    moisture_percent: 8.0,
                    dry_density_g_cm3: 1.80
                },
                CompactionPoint {
                    moisture_percent: 10.0,
                    dry_density_g_cm3: 1.90
                },
                CompactionPoint {
                    moisture_percent: 12.0,
                    dry_density_g_cm3: 2.00
                },
            ])
            .is_none()
        );
    }

    #[test]
    fn los_puntos_desordenados_se_ordenan_por_humedad() {
        let peak = curve_peak(&[
            CompactionPoint {
                moisture_percent: 12.0,
                dry_density_g_cm3: 1.90,
            },
            CompactionPoint {
                moisture_percent: 8.0,
                dry_density_g_cm3: 1.85,
            },
            CompactionPoint {
                moisture_percent: 10.0,
                dry_density_g_cm3: 1.95,
            },
        ])
        .unwrap();
        assert!(close(peak.optimum_moisture_percent, 10.333, 0.01));
    }
}
