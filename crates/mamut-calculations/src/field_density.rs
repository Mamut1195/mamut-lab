//! Densidad en sitio y grado de compactación.
//!
//! **Referencia pública:** TxDOT `Tex-115-E` §6.1 — la densidad seca en sitio se obtiene con
//! la misma relación que en laboratorio: `100 · γ_húmeda / (100 + w)`. Equivalencia de
//! referencia: ASTM D6938 (gauge nuclear) y D1556 (cono de arena).
//!
//! **Alcance.** Aquí solo se calcula. Cómo se obtuvo la densidad húmeda (gauge nuclear, cono
//! de arena, volumétrico) es un dato del ensayo que se registra aparte: el cálculo no puede
//! confundir métodos que dan resultados distintos.

use crate::compaction::dry_density;
use crate::{CalcError, Result};

/// Resultado de una determinación en sitio.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDensity {
    /// Densidad seca en sitio, en g/cm³.
    pub dry_density_g_cm3: f64,
    /// Grado de compactación respecto a la densidad seca máxima de laboratorio, en %.
    pub relative_compaction_percent: f64,
    /// Diferencia entre la humedad en sitio y la óptima, en puntos porcentuales.
    pub moisture_deviation_points: f64,
    /// Avisos de plausibilidad. Un resultado con avisos se informa, pero se revisa.
    pub warnings: Vec<&'static str>,
}

/// Calcula la densidad seca en sitio y el grado de compactación.
///
/// El grado de compactación se expresa respecto a la densidad seca **máxima** de la curva de
/// compactación de ese mismo material. Comparar contra otra curva no significa nada: por eso
/// la densidad máxima es un argumento obligatorio, no un valor por defecto.
///
/// Un grado de compactación por encima del 100 % o una humedad muy lejos de la óptima no se
/// rechazan —existen—, pero se marcan: casi siempre indican una densidad máxima equivocada,
/// sobretamaño en la muestra o unidades confundidas.
pub fn field_density(
    wet_density_g_cm3: f64,
    moisture_percent: f64,
    max_dry_density_g_cm3: f64,
    optimum_moisture_percent: f64,
) -> Result<FieldDensity> {
    if !wet_density_g_cm3.is_finite() || wet_density_g_cm3 <= 0.0 {
        return Err(CalcError::Incoherent(format!(
            "densidad húmeda no válida: {wet_density_g_cm3} g/cm³"
        )));
    }
    if !max_dry_density_g_cm3.is_finite() || max_dry_density_g_cm3 <= 0.0 {
        return Err(CalcError::Incoherent(
            "la densidad seca máxima de laboratorio es obligatoria para comparar".into(),
        ));
    }
    if !optimum_moisture_percent.is_finite() || optimum_moisture_percent < 0.0 {
        return Err(CalcError::Incoherent(format!(
            "humedad óptima no válida: {optimum_moisture_percent} %"
        )));
    }

    // Una sola implementación de la densidad seca: la de compactación.
    let dry = dry_density(wet_density_g_cm3, moisture_percent)?;
    let relative = 100.0 * dry / max_dry_density_g_cm3;
    let deviation = moisture_percent - optimum_moisture_percent;

    let mut warnings = Vec::new();
    if relative > 100.0 {
        warnings.push(
            "compactación por encima del 100 %: revisa la densidad máxima, el sobretamaño o las unidades",
        );
    } else if relative < 80.0 {
        warnings.push("compactación por debajo del 80 %: el material no está compactado");
    }
    if deviation.abs() > 3.0 {
        warnings.push("humedad a más de 3 puntos de la óptima");
    }

    Ok(FieldDensity {
        dry_density_g_cm3: dry,
        relative_compaction_percent: relative,
        moisture_deviation_points: deviation,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn densidad_seca_en_sitio() {
        // 2,1209 g/cm³ húmeda al 10 % → 1,9281 g/cm³ seca.
        let result = field_density(2.120_891, 10.0, 1.95, 11.0).unwrap();
        assert!(close(result.dry_density_g_cm3, 1.928_083, 1e-6));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn grado_de_compactacion_respecto_a_la_maxima() {
        let result = field_density(2.145, 10.0, 1.95, 10.0).unwrap();
        assert!(close(result.relative_compaction_percent, 100.0, 1e-6));
    }

    #[test]
    fn una_compactacion_insuficiente_da_menos_de_cien_y_avisa_si_es_muy_baja() {
        let result = field_density(1.98, 10.0, 1.95, 10.0).unwrap();
        assert!(result.relative_compaction_percent < 100.0);
        assert!(result.warnings.is_empty(), "98 % no es un aviso");

        let baja = field_density(1.40, 10.0, 1.95, 10.0).unwrap();
        assert!(!baja.warnings.is_empty(), "72 % sí lo es");
    }

    #[test]
    fn una_compactacion_imposible_avisa_en_vez_de_pasar_desapercibida() {
        // 117 % de compactación: densidad máxima equivocada o unidades confundidas.
        let result = field_density(2.40, 5.0, 1.95, 10.0).unwrap();
        assert!(result.relative_compaction_percent > 100.0);
        assert!(
            result.warnings.iter().any(|w| w.contains("100 %")),
            "{:?}",
            result.warnings
        );
        assert!(
            result.warnings.iter().any(|w| w.contains("humedad")),
            "{:?}",
            result.warnings
        );
    }

    #[test]
    fn la_desviacion_de_humedad_se_mide_en_puntos() {
        let result = field_density(2.1, 13.0, 1.95, 11.0).unwrap();
        assert!(close(result.moisture_deviation_points, 2.0, 1e-12));
        assert!(result.warnings.is_empty());

        let seco = field_density(2.1, 9.0, 1.95, 11.0).unwrap();
        assert!(close(seco.moisture_deviation_points, -2.0, 1e-12));

        let lejos = field_density(2.1, 20.0, 1.95, 11.0).unwrap();
        assert!(!lejos.warnings.is_empty(), "9 puntos sí es un aviso");
    }

    #[test]
    fn sin_densidad_maxima_no_hay_grado_de_compactacion() {
        assert!(field_density(2.1, 10.0, 0.0, 11.0).is_err());
    }

    #[test]
    fn datos_imposibles_se_rechazan() {
        assert!(field_density(0.0, 10.0, 1.95, 11.0).is_err());
        assert!(field_density(2.1, -1.0, 1.95, 11.0).is_err());
        assert!(field_density(2.1, 10.0, 1.95, -5.0).is_err());
    }
}
