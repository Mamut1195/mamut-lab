//! Límites de Atterberg e índice de plasticidad.
//!
//! **Referencias públicas:** TxDOT `Tex-104-E` (§8) para el límite líquido y `Tex-105-E`
//! (§5) para el plástico. Equivalencia de referencia: ASTM D4318 / AASHTO T 89 y T 90.
//! Análisis en `docs/normas/referencias-publicas/limites-atterberg.md`.
//!
//! **Una sola convención de humedad.** Todas las humedades de este módulo se calculan con las
//! masas del **recipiente** y su tara, que es como se pesa en el laboratorio:
//! `100 · (A − B) / (B − C)`, con A = recipiente + húmedo, B = recipiente + seco y C = tara.
//! La versión anterior recibía masas de suelo y no restaba la tara: daba 10 % donde la norma
//! da 13,33 % con las mismas pesadas.

use crate::{CalcError, Result};

/// Humedad de una muestra de suelo, con las masas del recipiente.
///
/// `100 · (A − B) / (B − C)` donde:
/// - `container_wet_g` (A) = recipiente con la muestra húmeda
/// - `container_dry_g` (B) = recipiente con la muestra seca
/// - `tare_g` (C) = recipiente vacío
pub fn moisture_percent(container_wet_g: f64, container_dry_g: f64, tare_g: f64) -> Result<f64> {
    if !tare_g.is_finite() || tare_g < 0.0 {
        return Err(CalcError::Incoherent(format!("tara no válida: {tare_g} g")));
    }
    let dry_soil = container_dry_g - tare_g;
    if !container_dry_g.is_finite() || dry_soil <= 0.0 {
        return Err(CalcError::Incoherent(format!(
            "la masa seca ({container_dry_g} g) debe superar la tara ({tare_g} g)"
        )));
    }
    let wet_soil = container_wet_g - tare_g;
    if !container_wet_g.is_finite() || wet_soil < dry_soil {
        return Err(CalcError::Incoherent(
            "la muestra húmeda no puede pesar menos que la seca".into(),
        ));
    }
    Ok(100.0 * (container_wet_g - container_dry_g) / dry_soil)
}

/// Límite líquido por el método de un punto.
///
/// `LL = W_N · (N / 25)^0,121`, con el exponente publicado por el método. La validez del
/// rango de golpes **no se comprueba aquí**: el método de referencia fija su propio rango y
/// esta función no lo conoce con certeza. Para eso está [`blows_within_typical_range`], que
/// informa sin bloquear.
pub fn liquid_limit_one_point(moisture_percent: f64, blows: u32) -> Result<f64> {
    if !moisture_percent.is_finite() || moisture_percent <= 0.0 {
        return Err(CalcError::Incoherent(format!(
            "humedad no válida para el límite líquido: {moisture_percent}"
        )));
    }
    if blows == 0 {
        return Err(CalcError::Incoherent(
            "el número de golpes debe ser mayor que cero".into(),
        ));
    }
    Ok(moisture_percent * (f64::from(blows) / 25.0).powf(0.121))
}

/// Indica si el número de golpes cae en el rango habitual del ensayo (10 a 40).
///
/// Es información, no una puerta: el rango exacto lo fija la norma de referencia y el
/// producto no puede imponerlo sin conocerla. Un resultado fuera de rango debe revisarse.
pub fn blows_within_typical_range(blows: u32) -> bool {
    (10..=40).contains(&blows)
}

/// Límite plástico: `PL = 100 · agua / (seco − tara)`.
pub fn plastic_limit(water_g: f64, container_dry_g: f64, tare_g: f64) -> Result<f64> {
    if !tare_g.is_finite() || tare_g < 0.0 {
        return Err(CalcError::Incoherent(format!("tara no válida: {tare_g} g")));
    }
    let soil = container_dry_g - tare_g;
    if !soil.is_finite() || soil <= 0.0 {
        return Err(CalcError::Incoherent(
            "la masa seca debe ser mayor que la tara".into(),
        ));
    }
    if !water_g.is_finite() || water_g < 0.0 {
        return Err(CalcError::Incoherent(format!(
            "masa de agua no válida: {water_g} g"
        )));
    }
    Ok(100.0 * water_g / soil)
}

/// Índice de plasticidad: `IP = LL − PL`.
///
/// Los dos límites deben ser positivos y el plástico no puede superar al líquido: cualquiera
/// de esas situaciones es un dato mal tomado, no un índice negativo.
pub fn plasticity_index(liquid_limit: f64, plastic_limit: f64) -> Result<f64> {
    if !liquid_limit.is_finite() || !plastic_limit.is_finite() {
        return Err(CalcError::Incoherent(
            "los límites deben ser números finitos".into(),
        ));
    }
    if liquid_limit <= 0.0 || plastic_limit <= 0.0 {
        return Err(CalcError::Incoherent(format!(
            "los límites deben ser positivos (LL {liquid_limit}, LP {plastic_limit})"
        )));
    }
    if plastic_limit > liquid_limit {
        return Err(CalcError::Incoherent(format!(
            "el límite plástico ({plastic_limit:.1}) no puede superar al líquido ({liquid_limit:.1})"
        )));
    }
    Ok(liquid_limit - plastic_limit)
}

/// Una determinación del límite líquido por el método multipunto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimitPoint {
    pub blows: u32,
    pub moisture_percent: f64,
}

/// Resultado del ajuste multipunto.
///
/// Se devuelven también la pendiente y el coeficiente de determinación: un ajuste con datos
/// dispersos tiene que poder marcarse como dudoso en lugar de presentarse como un número
/// limpio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiquidLimitFit {
    pub value: f64,
    pub slope: f64,
    pub r_squared: f64,
    pub determinations: usize,
    /// Alguna determinación fuera del rango habitual de golpes.
    pub has_blows_outside_range: bool,
}

/// Límite líquido por el método multipunto.
///
/// Ajuste lineal de la humedad frente al logaritmo de los golpes, evaluado en 25 golpes.
///
/// Se exigen **al menos tres** determinaciones: con dos el ajuste no tiene grados de libertad
/// y el resultado no se puede contrastar. La humedad tiene que **decrecer** al aumentar los
/// golpes; una pendiente no negativa indica un dato mal tomado y se rechaza.
pub fn liquid_limit_multi_point(points: &[LimitPoint]) -> Result<LiquidLimitFit> {
    if points.len() < 3 {
        return Err(CalcError::Incoherent(format!(
            "el método multipunto necesita al menos tres determinaciones (hay {})",
            points.len()
        )));
    }
    for point in points {
        if point.blows == 0 {
            return Err(CalcError::Incoherent(
                "hay una determinación con cero golpes".into(),
            ));
        }
        if !point.moisture_percent.is_finite() || point.moisture_percent <= 0.0 {
            return Err(CalcError::Incoherent(
                "hay una humedad no válida en las determinaciones".into(),
            ));
        }
    }

    let n = points.len() as f64;
    let xs: Vec<f64> = points.iter().map(|p| f64::from(p.blows).log10()).collect();
    let ys: Vec<f64> = points.iter().map(|p| p.moisture_percent).collect();
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;

    let mut covariance = 0.0;
    let mut variance = 0.0;
    for (x, y) in xs.iter().zip(ys.iter()) {
        covariance += (x - mean_x) * (y - mean_y);
        variance += (x - mean_x) * (x - mean_x);
    }
    if variance.abs() < f64::EPSILON {
        return Err(CalcError::Incoherent(
            "todas las determinaciones tienen el mismo número de golpes: no hay recta".into(),
        ));
    }

    let slope = covariance / variance;
    if slope >= 0.0 {
        return Err(CalcError::Incoherent(format!(
            "la humedad no decrece al aumentar los golpes (pendiente {slope:.2}): \
             revisa las determinaciones"
        )));
    }

    let intercept = mean_y - slope * mean_x;

    // Coeficiente de determinación: cuánto de la variación explica la recta.
    let total = ys.iter().map(|y| (y - mean_y) * (y - mean_y)).sum::<f64>();
    let residual = xs
        .iter()
        .zip(ys.iter())
        .map(|(x, y)| {
            let predicted = intercept + slope * x;
            (y - predicted) * (y - predicted)
        })
        .sum::<f64>();
    let r_squared = if total.abs() < f64::EPSILON {
        1.0
    } else {
        1.0 - residual / total
    };

    Ok(LiquidLimitFit {
        value: intercept + slope * 25f64.log10(),
        slope,
        r_squared,
        determinations: points.len(),
        has_blows_outside_range: points
            .iter()
            .any(|point| !blows_within_typical_range(point.blows)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn humedad_con_tara() {
        // A = 220 g (recipiente + húmedo), B = 200 g (recipiente + seco), C = 50 g (tara).
        // Referencia: (220 − 200) / (200 − 50) = 13,33 %.
        let value = moisture_percent(220.0, 200.0, 50.0).unwrap();
        assert!(close(value, 13.333, 0.001), "salió {value}");
    }

    #[test]
    fn la_humedad_sin_tara_daria_otro_numero() {
        // Con las mismas pesadas, ignorar la tara da 10 %: el error que se corrigió.
        let with_tare = moisture_percent(220.0, 200.0, 50.0).unwrap();
        let naive = 100.0 * (220.0 - 200.0) / 200.0;
        assert!((with_tare - naive).abs() > 3.0);
    }

    #[test]
    fn una_muestra_mas_seca_que_humeda_es_incoherente() {
        assert!(moisture_percent(190.0, 200.0, 50.0).is_err());
        assert!(moisture_percent(200.0, 200.0, 200.0).is_err());
    }

    #[test]
    fn el_metodo_de_un_punto_en_25_golpes_devuelve_la_humedad() {
        assert!(close(
            liquid_limit_one_point(40.0, 25).unwrap(),
            40.0,
            1e-12
        ));
    }

    #[test]
    fn el_metodo_de_un_punto_corrige_fuera_de_25_golpes() {
        let ll = liquid_limit_one_point(40.0, 36).unwrap();
        assert!(close(ll, 41.80, 0.02), "límite líquido calculado: {ll}");
        let ll_20 = liquid_limit_one_point(40.0, 20).unwrap();
        assert!(ll_20 < 40.0);
    }

    #[test]
    fn un_limite_liquido_sin_golpes_no_existe() {
        assert!(liquid_limit_one_point(40.0, 0).is_err());
        assert!(liquid_limit_one_point(0.0, 25).is_err());
    }

    #[test]
    fn el_rango_de_golpes_se_informa_sin_bloquear() {
        assert!(blows_within_typical_range(25));
        assert!(!blows_within_typical_range(7));
    }

    #[test]
    fn limite_plastico_a_partir_de_masas() {
        // Agua 5 g, seca 25 g, tara 5 g → 5 / 20 = 25 %.
        assert!(close(plastic_limit(5.0, 25.0, 5.0).unwrap(), 25.0, 1e-9));
    }

    #[test]
    fn una_tara_mayor_que_la_muestra_seca_es_incoherente() {
        assert!(plastic_limit(5.0, 10.0, 12.0).is_err());
    }

    #[test]
    fn el_indice_de_plasticidad_es_la_diferencia() {
        assert_eq!(plasticity_index(40.0, 25.0).unwrap(), 15.0);
    }

    #[test]
    fn un_limite_plastico_mayor_que_el_liquido_se_rechaza() {
        assert!(plasticity_index(30.0, 35.0).is_err());
    }

    #[test]
    fn limites_no_positivos_se_rechazan() {
        assert!(plasticity_index(-5.0, -10.0).is_err());
        assert!(plasticity_index(0.0, 0.0).is_err());
        assert!(plasticity_index(40.0, 0.0).is_err());
    }

    #[test]
    fn el_metodo_multipunto_da_un_valor_entre_las_determinaciones() {
        let fit = liquid_limit_multi_point(&[
            LimitPoint {
                blows: 20,
                moisture_percent: 45.0,
            },
            LimitPoint {
                blows: 30,
                moisture_percent: 41.0,
            },
            LimitPoint {
                blows: 40,
                moisture_percent: 38.0,
            },
        ])
        .unwrap();
        assert!(
            (41.0..45.0).contains(&fit.value),
            "el límite a 25 golpes debe caer entre las determinaciones: {}",
            fit.value
        );
        assert!(fit.slope < 0.0);
        assert!(fit.r_squared > 0.9, "el ajuste es bueno: {}", fit.r_squared);
        assert_eq!(fit.determinations, 3);
        assert!(!fit.has_blows_outside_range);
    }

    #[test]
    fn dos_puntos_no_bastan() {
        let error = liquid_limit_multi_point(&[
            LimitPoint {
                blows: 20,
                moisture_percent: 44.0,
            },
            LimitPoint {
                blows: 30,
                moisture_percent: 40.0,
            },
        ])
        .unwrap_err();
        assert!(error.to_string().contains("tres"));
    }

    #[test]
    fn una_pendiente_no_negativa_es_un_dato_mal_tomado() {
        let error = liquid_limit_multi_point(&[
            LimitPoint {
                blows: 20,
                moisture_percent: 38.0,
            },
            LimitPoint {
                blows: 30,
                moisture_percent: 41.0,
            },
            LimitPoint {
                blows: 40,
                moisture_percent: 44.0,
            },
        ])
        .unwrap_err();
        assert!(error.to_string().contains("decrece"));
    }

    #[test]
    fn se_informa_si_hay_golpes_fuera_de_rango() {
        let fit = liquid_limit_multi_point(&[
            LimitPoint {
                blows: 7,
                moisture_percent: 50.0,
            },
            LimitPoint {
                blows: 15,
                moisture_percent: 45.0,
            },
            LimitPoint {
                blows: 25,
                moisture_percent: 41.0,
            },
        ])
        .unwrap();
        assert!(fit.has_blows_outside_range);
    }

    #[test]
    fn el_multipunto_exige_golpes_distintos() {
        assert!(
            liquid_limit_multi_point(&[
                LimitPoint {
                    blows: 25,
                    moisture_percent: 40.0
                },
                LimitPoint {
                    blows: 25,
                    moisture_percent: 41.0
                },
                LimitPoint {
                    blows: 25,
                    moisture_percent: 42.0
                },
            ])
            .is_err()
        );
    }
}
