//! Granulometría por tamices.
//!
//! **Referencia pública:** TxDOT `Tex-110-E`, sección 5 (*Calculations*), edición abril
//! 2024. Equivalencia de referencia: ASTM D6913 / AASHTO T 27.
//! Análisis y decisiones en `docs/normas/referencias-publicas/granulometria.md`.
//!
//! **Qué se calcula.** El porcentaje retenido acumulado en cada tamiz, su complemento (el
//! que pasa) y los diámetros característicos D10, D30 y D60 con los coeficientes de
//! uniformidad y curvatura. Nada de esto decide si el suelo es aceptable: eso es
//! conformidad y vive en otro crate.

use crate::{CalcError, Result};

/// Una malla con lo que se retuvo en ella.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SievePass {
    /// Abertura del tamiz en milímetros.
    pub size_mm: f64,
    /// Masa retenida en ese tamiz, en gramos.
    pub retained_g: f64,
}

/// Resultado del análisis granulométrico.
#[derive(Clone, Debug, PartialEq)]
pub struct GranulometryOutcome {
    pub total_dry_g: f64,
    /// Cuántos tamices sostienen la curva.
    pub sieve_count: usize,
    /// Algún diámetro característico cayó exactamente en un extremo de la curva, es decir,
    /// sin intervalo de interpolación por debajo o por encima.
    pub d_at_boundary: bool,
    /// `(abertura_mm, % que pasa)`, de grueso a fino.
    pub passing: Vec<(f64, f64)>,
    /// `(abertura_mm, % retenido acumulado)`, de grueso a fino.
    pub cumulative_retained: Vec<(f64, f64)>,
    pub d10: Option<f64>,
    pub d30: Option<f64>,
    pub d60: Option<f64>,
    /// `D60 / D10`.
    pub cu: Option<f64>,
    /// `D30² / (D10 · D60)`.
    pub cc: Option<f64>,
}

/// Calcula la granulometría a partir de la masa seca total y lo retenido en cada tamiz.
///
/// El porcentaje que pasa es el complemento del retenido acumulado, que es lo que publica
/// el método de referencia.
pub fn sieve_analysis(total_dry_g: f64, sieves: &[SievePass]) -> Result<GranulometryOutcome> {
    if !total_dry_g.is_finite() || total_dry_g <= 0.0 {
        return Err(CalcError::Core(mamut_core::Error::NotPositive {
            field: "masa seca total",
            value: total_dry_g,
        }));
    }
    if sieves.is_empty() {
        return Err(CalcError::Incoherent(
            "no hay tamices: un análisis granulométrico necesita al menos uno".into(),
        ));
    }

    let mut ordered = sieves.to_vec();
    for sieve in &ordered {
        if !sieve.size_mm.is_finite() || sieve.size_mm <= 0.0 {
            return Err(CalcError::Incoherent(format!(
                "abertura de tamiz no válida: {} mm",
                sieve.size_mm
            )));
        }
        if !sieve.retained_g.is_finite() || sieve.retained_g < 0.0 {
            return Err(CalcError::Incoherent(format!(
                "masa retenida no válida en {} mm: {} g",
                sieve.size_mm, sieve.retained_g
            )));
        }
    }
    // De grueso a fino: así el acumulado se calcula en el orden correcto.
    ordered.sort_by(|a, b| b.size_mm.total_cmp(&a.size_mm));

    // Dos filas con la misma abertura harían que un tamiz apareciera dos veces en la curva.
    for pair in ordered.windows(2) {
        if pair[0].size_mm == pair[1].size_mm {
            return Err(CalcError::Incoherent(format!(
                "el tamiz de {} mm está repetido",
                pair[0].size_mm
            )));
        }
    }

    let sum: f64 = ordered.iter().map(|s| s.retained_g).sum();
    // Tolerancia de pesada: la balanza redondea a 0,1 g y con varios tamices el redondeo se
    // acumula. Un exceso dentro de esa tolerancia no es un dato imposible.
    let tolerance = (total_dry_g * 0.002).max(0.1);
    if sum > total_dry_g + tolerance {
        return Err(CalcError::Incoherent(format!(
            "lo retenido ({sum:.2} g) supera la masa seca total ({total_dry_g:.2} g) \
             en más de la tolerancia de pesada ({tolerance:.2} g)"
        )));
    }

    let mut cumulative_retained = Vec::with_capacity(ordered.len());
    let mut passing = Vec::with_capacity(ordered.len());
    let mut running = 0.0;
    for sieve in &ordered {
        running += sieve.retained_g;
        let cumulative = 100.0 * running / total_dry_g;
        cumulative_retained.push((sieve.size_mm, cumulative));
        // Un exceso de pesada dentro de la tolerancia puede dejar el complemento en negativo.
        // Se acota a cero: un porcentaje que pasa negativo no existe.
        passing.push((sieve.size_mm, (100.0 - cumulative).max(0.0)));
    }

    let d10 = size_at_percent(&passing, 10.0);
    let d30 = size_at_percent(&passing, 30.0);
    let d60 = size_at_percent(&passing, 60.0);

    let cu = match (d60, d10) {
        (Some(d60), Some(d10)) if d10 > 0.0 => Some(d60 / d10),
        _ => None,
    };
    let cc = match (d30, d10, d60) {
        (Some(d30), Some(d10), Some(d60)) if d10 > 0.0 && d60 > 0.0 => {
            Some(d30 * d30 / (d10 * d60))
        }
        _ => None,
    };

    let finest = passing.last().map(|(size, _)| *size);
    let coarsest = passing.first().map(|(size, _)| *size);
    let d_at_boundary = [d10, d30, d60].iter().flatten().any(|value| {
        coarsest.is_some_and(|limit| (value - limit).abs() < 1e-12)
            || finest.is_some_and(|limit| (value - limit).abs() < 1e-12)
    });

    Ok(GranulometryOutcome {
        total_dry_g,
        sieve_count: ordered.len(),
        d_at_boundary,
        passing,
        cumulative_retained,
        d10,
        d30,
        d60,
        cu,
        cc,
    })
}

/// Diámetro para el que pasa un `target` por ciento, interpolando en escala logarítmica.
///
/// **Mesetas.** Si el porcentaje no cambia entre dos tamices (meseta), el diámetro es
/// ambiguo; se devuelve el más fino de los dos. Es una elección, no un resultado de la
/// geometría, y por eso está escrita aquí.
///
/// La interpolación es logarítmica porque la granulometría se representa así: entre 2 mm y
/// 0,075 mm la distancia que importa es la de las potencias de diez, no la lineal.
fn size_at_percent(passing: &[(f64, f64)], target: f64) -> Option<f64> {
    // `passing` viene de grueso a fino, así que el porcentaje decrece.
    for pair in passing.windows(2) {
        let (coarse_size, coarse_passing) = pair[0];
        let (fine_size, fine_passing) = pair[1];
        if coarse_passing >= target && fine_passing <= target {
            if (coarse_passing - fine_passing).abs() < f64::EPSILON {
                return Some(fine_size);
            }
            let t = (target - coarse_passing) / (fine_passing - coarse_passing);
            let log_size = coarse_size.log10() + t * (fine_size.log10() - coarse_size.log10());
            return Some(10f64.powf(log_size));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn porcentaje_que_pasa_es_el_complemento_del_retenido() {
        // 1000 g; retenidos 100, 200 y 300 → acumulados 10 %, 30 % y 60 %.
        let outcome = sieve_analysis(
            1000.0,
            &[
                SievePass {
                    size_mm: 4.75,
                    retained_g: 100.0,
                },
                SievePass {
                    size_mm: 2.0,
                    retained_g: 200.0,
                },
                SievePass {
                    size_mm: 0.425,
                    retained_g: 300.0,
                },
            ],
        )
        .unwrap();

        assert_eq!(outcome.cumulative_retained[0], (4.75, 10.0));
        assert_eq!(outcome.cumulative_retained[1], (2.0, 30.0));
        assert_eq!(outcome.cumulative_retained[2], (0.425, 60.0));
        assert_eq!(outcome.passing[0], (4.75, 90.0));
        assert_eq!(outcome.passing[2], (0.425, 40.0));
    }

    #[test]
    fn el_orden_de_los_tamices_no_altera_el_resultado() {
        let desordenado = sieve_analysis(
            1000.0,
            &[
                SievePass {
                    size_mm: 0.425,
                    retained_g: 300.0,
                },
                SievePass {
                    size_mm: 4.75,
                    retained_g: 100.0,
                },
                SievePass {
                    size_mm: 2.0,
                    retained_g: 200.0,
                },
            ],
        )
        .unwrap();
        assert_eq!(desordenado.passing[0], (4.75, 90.0));
        assert_eq!(desordenado.passing[2], (0.425, 40.0));
    }

    #[test]
    fn d10_d30_d60_y_coeficientes_con_una_curva_conocida() {
        // Retenidos que dan % que pasa: 80, 60, 30, 10.
        let outcome = sieve_analysis(
            1000.0,
            &[
                SievePass {
                    size_mm: 2.0,
                    retained_g: 200.0,
                },
                SievePass {
                    size_mm: 0.6,
                    retained_g: 200.0,
                },
                SievePass {
                    size_mm: 0.2,
                    retained_g: 300.0,
                },
                SievePass {
                    size_mm: 0.075,
                    retained_g: 200.0,
                },
            ],
        )
        .unwrap();

        assert!(close(outcome.d60.unwrap(), 0.6, 1e-12));
        assert!(close(outcome.d30.unwrap(), 0.2, 1e-12));
        assert!(close(outcome.d10.unwrap(), 0.075, 1e-12));
        // Cu = 0.6 / 0.075 = 8
        assert!(close(outcome.cu.unwrap(), 8.0, 1e-9));
        // Cc = 0.2² / (0.075 · 0.6) = 0.888…
        assert!(close(outcome.cc.unwrap(), 0.888_888_888, 1e-6));
    }

    #[test]
    fn un_punto_intermedio_se_interpola_en_escala_logaritmica() {
        // Pasa 100 % en 2 mm y 0 % en 0,02 mm: el 50 % debe caer en la media geométrica.
        let outcome = sieve_analysis(
            100.0,
            &[
                SievePass {
                    size_mm: 2.0,
                    retained_g: 0.0,
                },
                SievePass {
                    size_mm: 0.02,
                    retained_g: 100.0,
                },
            ],
        )
        .unwrap();
        let d50 = size_at_percent(&outcome.passing, 50.0).unwrap();
        assert!(close(d50, 0.2, 1e-9), "media geométrica de 2 y 0,02: {d50}");
    }

    #[test]
    fn la_interpolacion_es_asimetrica_y_logaritmica() {
        // Pasa 70 % en 2 mm y 10 % en 0,425 mm. Un objetivo intermedio obliga a interpolar
        // en el intervalo: si la interpolación fuera lineal o el parámetro estuviera
        // invertido, estos valores no saldrían.
        let outcome = sieve_analysis(
            1000.0,
            &[
                SievePass {
                    size_mm: 2.0,
                    retained_g: 300.0,
                },
                SievePass {
                    size_mm: 0.425,
                    retained_g: 600.0,
                },
            ],
        )
        .unwrap();

        let d60 = outcome.d60.unwrap();
        let d30 = outcome.d30.unwrap();
        assert!((d60 - 1.545).abs() < 0.005, "D60 = {d60}");
        assert!((d30 - 0.712).abs() < 0.005, "D30 = {d30}");
        assert!(d60 > d30, "el diámetro crece con el porcentaje que pasa");
    }

    #[test]
    fn un_diametro_en_el_borde_de_la_curva_se_marca() {
        let outcome = sieve_analysis(
            100.0,
            &[
                SievePass {
                    size_mm: 2.0,
                    retained_g: 10.0,
                },
                SievePass {
                    size_mm: 0.075,
                    retained_g: 80.0,
                },
            ],
        )
        .unwrap();
        assert_eq!(outcome.sieve_count, 2);
        // El 10 % cae justo en el tamiz más fino: el diámetro existe, pero es el borde de
        // la curva y así queda marcado.
        assert!(outcome.d10.is_some());
        assert!(outcome.d_at_boundary);
    }

    #[test]
    fn lo_retenido_dentro_de_la_tolerancia_de_pesada_se_acepta() {
        // 500,05 g retenidos sobre 500,0 g: es el redondeo de la balanza, no un imposible.
        let outcome = sieve_analysis(
            500.0,
            &[
                SievePass {
                    size_mm: 4.75,
                    retained_g: 250.1,
                },
                SievePass {
                    size_mm: 2.0,
                    retained_g: 249.95,
                },
            ],
        )
        .unwrap();
        assert_eq!(outcome.sieve_count, 2);
        // El complemento se acota a cero: no hay porcentajes que pasen negativos.
        assert_eq!(outcome.passing[1].1, 0.0);
    }

    #[test]
    fn una_masa_total_nula_o_negativa_no_es_un_analisis() {
        assert!(
            sieve_analysis(
                0.0,
                &[SievePass {
                    size_mm: 2.0,
                    retained_g: 1.0
                }]
            )
            .is_err()
        );
        assert!(
            sieve_analysis(
                -5.0,
                &[SievePass {
                    size_mm: 2.0,
                    retained_g: 1.0
                }]
            )
            .is_err()
        );
    }

    #[test]
    fn lo_retenido_no_puede_superar_la_muestra() {
        let error = sieve_analysis(
            100.0,
            &[SievePass {
                size_mm: 2.0,
                retained_g: 150.0,
            }],
        )
        .unwrap_err();
        assert!(error.to_string().contains("supera la masa seca total"));
    }

    #[test]
    fn un_tamiz_negativo_o_una_masa_negativa_se_rechazan() {
        assert!(
            sieve_analysis(
                100.0,
                &[SievePass {
                    size_mm: -2.0,
                    retained_g: 1.0
                }]
            )
            .is_err()
        );
        assert!(
            sieve_analysis(
                100.0,
                &[SievePass {
                    size_mm: 2.0,
                    retained_g: -1.0
                }]
            )
            .is_err()
        );
    }

    #[test]
    fn sin_tamices_no_hay_analisis() {
        assert!(sieve_analysis(100.0, &[]).is_err());
    }

    #[test]
    fn un_tamiz_repetido_se_rechaza() {
        let error = sieve_analysis(
            100.0,
            &[
                SievePass {
                    size_mm: 2.0,
                    retained_g: 10.0,
                },
                SievePass {
                    size_mm: 2.0,
                    retained_g: 20.0,
                },
            ],
        )
        .unwrap_err();
        assert!(error.to_string().contains("repetido"));
    }

    #[test]
    fn una_curva_sin_los_diámetros_no_inventa_coeficientes() {
        // Solo un tamiz: no hay D10 ni D60.
        let outcome = sieve_analysis(
            100.0,
            &[SievePass {
                size_mm: 2.0,
                retained_g: 10.0,
            }],
        )
        .unwrap();
        assert!(outcome.d10.is_none());
        assert!(outcome.cu.is_none());
    }
}
