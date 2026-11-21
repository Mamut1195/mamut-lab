//! Límites de Atterberg e índice de plasticidad.
//!
//! **Referencias públicas:** TxDOT `Tex-104-E` (§8) para el límite líquido y `Tex-105-E`
//! (§5) para el plástico. Equivalencia de referencia: ASTM D4318 / AASHTO T 89 y T 90.
//! Análisis en `docs/normas/referencias-publicas/limites-atterberg.md`.

use crate::{CalcError, Result};

fn finite_positive(value: f64, field: &'static str) -> Result<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(CalcError::Core(mamut_core::Error::NotPositive {
            field,
            value,
        }));
    }
    Ok(value)
}

/// Humedad de una muestra: `100 · (húmedo − seco) / seco`.
pub fn moisture_percent(wet_g: f64, dry_g: f64) -> Result<f64> {
    let dry = finite_positive(dry_g, "masa seca")?;
    if !wet_g.is_finite() || wet_g < 0.0 {
        return Err(CalcError::Incoherent(format!(
            "masa húmeda no válida: {wet_g} g"
        )));
    }
    if wet_g < dry {
        return Err(CalcError::Incoherent(
            "la masa húmeda no puede ser menor que la seca".into(),
        ));
    }
    Ok(100.0 * (wet_g - dry) / dry)
}

/// Límite líquido por el método de un punto.
///
/// `LL = W_N · (N / 25)^0,121`, con el exponente publicado por el método.
/// La condición de validez del rango de golpes la fija la norma de referencia y se comprueba
/// en la capa de validación, no aquí.
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

/// Límite plástico: `PL = 100 · agua / (seco − tara)`.
pub fn plastic_limit(water_g: f64, dry_g: f64, tare_g: f64) -> Result<f64> {
    if !tare_g.is_finite() || tare_g < 0.0 {
        return Err(CalcError::Incoherent(format!("tara no válida: {tare_g} g")));
    }
    let soil = dry_g - tare_g;
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
/// Si el límite plástico supera al líquido, los datos son incoherentes: se devuelve error en
/// lugar de un número negativo que acabaría en un informe.
pub fn plasticity_index(liquid_limit: f64, plastic_limit: f64) -> Result<f64> {
    if !liquid_limit.is_finite() || !plastic_limit.is_finite() {
        return Err(CalcError::Incoherent(
            "los límites deben ser números finitos".into(),
        ));
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

/// Límite líquido por el método multipunto.
///
/// Ajuste lineal de la humedad frente al logaritmo de los golpes y evaluación en 25 golpes.
/// Es el procedimiento del método: la curva es una recta en escala semilogarítmica.
pub fn liquid_limit_multi_point(points: &[LimitPoint]) -> Result<f64> {
    if points.len() < 2 {
        return Err(CalcError::Incoherent(
            "el método multipunto necesita al menos dos determinaciones".into(),
        ));
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
    let intercept = mean_y - slope * mean_x;
    Ok(intercept + slope * 25f64.log10())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn humedad_de_una_muestra() {
        // 220 g húmedos, 200 g secos → 10 %.
        assert!(close(moisture_percent(220.0, 200.0).unwrap(), 10.0, 1e-9));
    }

    #[test]
    fn una_muestra_mas_seca_que_humeda_es_incoherente() {
        assert!(moisture_percent(190.0, 200.0).is_err());
        assert!(moisture_percent(200.0, 0.0).is_err());
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
        // 40 % a 36 golpes: 40 · (36/25)^0,121 ≈ 41,80 %.
        let ll = liquid_limit_one_point(40.0, 36).unwrap();
        assert!(close(ll, 41.80, 0.02), "límite líquido calculado: {ll}");
        // Menos golpes que 25 baja el límite: el suelo está más húmedo.
        let ll_20 = liquid_limit_one_point(40.0, 20).unwrap();
        assert!(ll_20 < 40.0);
    }

    #[test]
    fn un_limite_liquido_sin_golpes_no_existe() {
        assert!(liquid_limit_one_point(40.0, 0).is_err());
        assert!(liquid_limit_one_point(0.0, 25).is_err());
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
    fn el_metodo_multipunto_da_un_valor_entre_las_determinaciones() {
        // Tres determinaciones coherentes: más golpes, menos humedad.
        let ll = liquid_limit_multi_point(&[
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
            (41.0..45.0).contains(&ll),
            "el límite a 25 golpes debe caer entre las determinaciones: {ll}"
        );
    }

    #[test]
    fn con_dos_puntos_tambien_hay_recta() {
        let ll = liquid_limit_multi_point(&[
            LimitPoint {
                blows: 20,
                moisture_percent: 44.0,
            },
            LimitPoint {
                blows: 30,
                moisture_percent: 40.0,
            },
        ])
        .unwrap();
        assert!((40.0..44.0).contains(&ll));
    }

    #[test]
    fn el_multipunto_necesita_dos_determinaciones_y_golpes_distintos() {
        assert!(liquid_limit_multi_point(&[]).is_err());
        assert!(
            liquid_limit_multi_point(&[LimitPoint {
                blows: 25,
                moisture_percent: 40.0
            }])
            .is_err()
        );
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
            ])
            .is_err(),
            "sin golpes distintos no hay recta"
        );
    }
}
