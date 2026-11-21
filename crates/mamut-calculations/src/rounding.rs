//! Redondeo para **presentación**, nunca para comparación.
//!
//! Comparar un valor ya redondeado contra un límite es un error de ingeniería clásico:
//! `20.9996` se presenta como `21.0` y parecería conforme, pero no lo es. El orden
//! correcto es: comparar con el valor sin redondear, y redondear solo al mostrar.
//!
//! El número de decimales es una **política configurable**. La cifra normativa de cada
//! ensayo se fijará al revisar la norma correspondiente (ADR-0009): hasta entonces, la
//! política por defecto es una decisión de presentación, no una afirmación normativa.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundingPolicy {
    pub decimals: u8,
}

impl RoundingPolicy {
    /// Máximo de decimales que tiene sentido pedir: por encima, el factor de escala deja
    /// de ser representable y el redondeo devolvería `NaN` para una entrada finita.
    pub const MAX_DECIMALS: u8 = 15;

    /// Política por defecto del producto: dos decimales.
    ///
    /// **Provisional.** No se presenta como la cifra significativa de ninguna norma.
    pub const PROVISIONAL: Self = Self { decimals: 2 };

    pub fn new(decimals: u8) -> Self {
        Self {
            decimals: decimals.min(Self::MAX_DECIMALS),
        }
    }

    pub fn apply(self, value: f64) -> f64 {
        round_half_away_from_zero(value, self.decimals)
    }
}

impl Default for RoundingPolicy {
    fn default() -> Self {
        Self::PROVISIONAL
    }
}

/// Redondeo «mitad fuera de cero» (el que usa el común de los mortales al leer un
/// número), a `decimals` decimales.
pub fn round_half_away_from_zero(value: f64, decimals: u8) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let factor = 10f64.powi(i32::from(decimals.min(RoundingPolicy::MAX_DECIMALS)));
    if !factor.is_finite() || factor == 0.0 {
        // Sin factor válido no se puede redondear: se devuelve el valor intacto antes que
        // inventar un NaN.
        return value;
    }
    let scaled = value * factor;
    // Si la escala desborda, devolver el valor tal cual es mejor que devolver infinito.
    if !scaled.is_finite() {
        return value;
    }
    let rounded = if scaled >= 0.0 {
        (scaled + 0.5).floor()
    } else {
        (scaled - 0.5).ceil()
    };
    rounded / factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redondea_a_dos_decimales() {
        assert_eq!(round_half_away_from_zero(28.294_212_1, 2), 28.29);
        assert_eq!(round_half_away_from_zero(28.295_0, 2), 28.30);
    }

    #[test]
    fn redondea_mitades_fuera_de_cero() {
        assert_eq!(round_half_away_from_zero(2.5, 0), 3.0);
        assert_eq!(round_half_away_from_zero(-2.5, 0), -3.0);
    }

    #[test]
    fn un_valor_ya_redondeado_no_cambia() {
        assert_eq!(round_half_away_from_zero(21.0, 2), 21.0);
    }

    #[test]
    fn nan_e_infinito_no_se_tocan() {
        assert!(round_half_away_from_zero(f64::NAN, 2).is_nan());
        assert!(round_half_away_from_zero(f64::INFINITY, 2).is_infinite());
    }

    #[test]
    fn pedir_demasiados_decimales_no_produce_nan() {
        // `10f64.powi(309)` es infinito y `inf/inf` daría NaN para una entrada finita.
        for decimals in [16u8, 40, 255] {
            let result = round_half_away_from_zero(1.234, decimals);
            assert!(
                result.is_finite(),
                "con {decimals} decimales el resultado debe seguir siendo finito, fue {result}"
            );
        }
        let policy = RoundingPolicy::new(200);
        assert_eq!(policy.decimals, RoundingPolicy::MAX_DECIMALS);
        assert!(policy.apply(1.234).is_finite());
    }

    #[test]
    fn el_peligro_de_comparar_redondeado() {
        // Este es el motivo de que la comparación use el valor sin redondear:
        // 20.9996 se presenta como 21.00, pero NO alcanza el mínimo de 21.0.
        let valor = 20.999_6;
        assert_eq!(
            RoundingPolicy::new(2).apply(valor),
            21.0,
            "se presenta como 21.00"
        );
        assert!(
            valor < 21.0,
            "pero el valor real está por debajo del mínimo: comparar el redondeado daría un falso conforme"
        );
    }

    #[test]
    fn el_artefacto_binario_esta_documentado() {
        // 1,005 y 0,145 no son exactos en binario: valen 1,00499999… y 0,14499999…, así que
        // el redondeo "hacia afuera" no los sube. Se documenta el comportamiento real en vez
        // de prometer un decimal exacto que la representación no permite.
        assert_eq!(round_half_away_from_zero(1.005, 2), 1.0);
        assert_eq!(round_half_away_from_zero(0.145, 2), 0.14);
        // Los casos que sí son exactos se redondean como se espera.
        assert_eq!(round_half_away_from_zero(2.345, 2), 2.35);
    }
}
