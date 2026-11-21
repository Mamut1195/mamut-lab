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

    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    // Sumas para las ecuaciones normales: S0..S4 de x, T0..T2 de y·x^k.
    let mut s = [0.0_f64; 5];
    let mut t = [0.0_f64; 3];

    for point in points {
        let x = point.moisture_percent;
        let y = point.dry_density_g_cm3;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        x_min = x_min.min(x);
        x_max = x_max.max(x);
        y_max = y_max.max(y);

        let mut power = 1.0;
        for slot in s.iter_mut() {
            *slot += power;
            power *= x;
        }
        let mut power = 1.0;
        for slot in t.iter_mut() {
            *slot += y * power;
            power *= x;
        }
    }

    if x_max <= x_min {
        return None;
    }

    // Sistema 3x3 por la regla de Cramer.
    let det = s[4] * (s[2] * s[0] - s[1] * s[1]) - s[3] * (s[3] * s[0] - s[1] * s[2])
        + s[2] * (s[3] * s[1] - s[2] * s[2]);
    if det.abs() < 1e-12 || !det.is_finite() {
        return None;
    }
    let det_a = t[2] * (s[2] * s[0] - s[1] * s[1]) - s[3] * (t[1] * s[0] - s[1] * t[0])
        + s[2] * (t[1] * s[1] - s[2] * t[0]);
    let det_b = s[4] * (t[1] * s[0] - s[1] * t[0]) - t[2] * (s[3] * s[0] - s[1] * s[2])
        + s[2] * (s[3] * t[0] - t[1] * s[2]);
    let det_c = s[4] * (s[2] * t[0] - t[1] * s[1]) - s[3] * (s[3] * t[0] - t[1] * s[2])
        + t[2] * (s[3] * s[1] - s[2] * s[2]);

    let a = det_a / det;
    let b = det_b / det;
    let c = det_c / det;
    if !a.is_finite() || !b.is_finite() || !c.is_finite() || a >= 0.0 {
        return None;
    }

    let vertex_x = -b / (2.0 * a);
    let vertex_y = a * vertex_x * vertex_x + b * vertex_x + c;

    if !vertex_x.is_finite() || !vertex_y.is_finite() {
        return None;
    }
    if vertex_x < x_min || vertex_x > x_max {
        return None;
    }
    // El máximo no puede superar lo medido mas de un 1 %, ni quedarse por debajo.
    // Solo se acota por arriba: un vertice por debajo del maximo medido es lo normal cuando
    // hay dispersion entre puntos. Acotarlo por abajo dejaba sin resultado curvas validas.
    if vertex_y > y_max * 1.01 {
        return None;
    }

    Some(CurvePeak {
        optimum_moisture_percent: vertex_x,
        max_dry_density_g_cm3: vertex_y,
        points_used: points.len(),
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
    fn el_maximo_de_la_curva_con_tres_puntos() {
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

    #[test]
    fn un_punto_lejano_no_infla_la_densidad_maxima() {
        // Con (8)(10)(20) la parábola de los tres puntos situaba el máximo en 2,021 g/cm³,
        // un 3,6 % por encima de todo lo medido. Debe rechazarse.
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
                CompactionPoint {
                    moisture_percent: 20.0,
                    dry_density_g_cm3: 1.80
                },
            ])
            .is_none(),
            "un máximo por encima de lo medido no es un resultado"
        );
    }

    #[test]
    fn el_ajuste_usa_todos_los_puntos_y_no_solo_tres() {
        // Cinco puntos: el ajuste por mínimos cuadrados da 12,16 % y 1,9473 g/cm³.
        let peak = curve_peak(&[
            CompactionPoint {
                moisture_percent: 6.0,
                dry_density_g_cm3: 1.80,
            },
            CompactionPoint {
                moisture_percent: 8.0,
                dry_density_g_cm3: 1.87,
            },
            CompactionPoint {
                moisture_percent: 10.0,
                dry_density_g_cm3: 1.95,
            },
            CompactionPoint {
                moisture_percent: 13.0,
                dry_density_g_cm3: 1.93,
            },
            CompactionPoint {
                moisture_percent: 17.0,
                dry_density_g_cm3: 1.86,
            },
        ])
        .unwrap();
        assert!(
            close(peak.optimum_moisture_percent, 12.16, 0.02),
            "humedad óptima {}",
            peak.optimum_moisture_percent
        );
        assert!(
            close(peak.max_dry_density_g_cm3, 1.9473, 0.001),
            "densidad máxima {}",
            peak.max_dry_density_g_cm3
        );
        assert_eq!(peak.points_used, 5, "todos los puntos sostienen el ajuste");
    }

    #[test]
    fn un_valor_no_finito_no_produce_maximo() {
        assert!(
            curve_peak(&[
                CompactionPoint {
                    moisture_percent: 8.0,
                    dry_density_g_cm3: f64::NAN
                },
                CompactionPoint {
                    moisture_percent: 10.0,
                    dry_density_g_cm3: 1.95
                },
                CompactionPoint {
                    moisture_percent: 12.0,
                    dry_density_g_cm3: 1.90
                },
            ])
            .is_none()
        );
    }

    #[test]
    fn una_curva_con_dispersion_normal_si_produce_maximo() {
        // Cinco puntos con un pico que sobresale un 1,2 % de la parábola: es dispersión
        // normal de laboratorio, no un dato malo. Rechazarla dejaba el ensayo sin resultado.
        let peak = curve_peak(&[
            CompactionPoint {
                moisture_percent: 8.0,
                dry_density_g_cm3: 1.87,
            },
            CompactionPoint {
                moisture_percent: 10.0,
                dry_density_g_cm3: 1.96,
            },
            CompactionPoint {
                moisture_percent: 12.0,
                dry_density_g_cm3: 1.92,
            },
            CompactionPoint {
                moisture_percent: 14.0,
                dry_density_g_cm3: 1.90,
            },
            CompactionPoint {
                moisture_percent: 16.0,
                dry_density_g_cm3: 1.87,
            },
        ])
        .expect("una curva normal debe dar maximo");
        assert!(peak.max_dry_density_g_cm3 < 1.96 * 1.01);
        assert_eq!(peak.points_used, 5);
    }
}
