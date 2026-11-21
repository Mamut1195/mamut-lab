//! Magnitudes con identidad de dimensión.
//!
//! **Por qué tipos propios y no `f64`.** Un `f64` no dice si son milímetros o metros,
//! ni newtons o kilonewtons. Mezclarlos produce resultados incorrectos que compilan.
//! Aquí cada dimensión es un tipo y cada conversión es explícita.
//!
//! **Representación interna.** Todo se guarda en unidades del SI (m, kg, N, m², Pa,
//! % ; grados Celsius). El valor canónico se lee con [`Length::canonical`] y similares;
//! las conversiones usan métodos con nombre (`from_millimetres`, `millimetres`).
//!
//! **No existe `From<f64>` a propósito**: construir una magnitud exige elegir la unidad.

use crate::error::{Error, Result};

/// Rechaza NaN e infinito. Una magnitud sin valor finito no es una magnitud.
fn finite(value: f64, field: &'static str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::NonFinite { field })
    }
}

/// Rechaza NaN, infinito y valores no positivos.
fn positive(value: f64, field: &'static str) -> Result<f64> {
    let value = finite(value, field)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(Error::NotPositive { field, value })
    }
}

macro_rules! magnitude {
    ($name:ident, $field:literal, $unit:literal) => {
        impl $name {
            /// Valor en unidades del SI.
            pub fn canonical(self) -> f64 {
                self.0
            }

            /// Si la magnitud es estrictamente mayor que cero.
            pub fn is_positive(self) -> bool {
                self.0 > 0.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{} {}", self.0, $unit)
            }
        }
    };
}

// ---------------------------------------------------------------- longitud ---

/// Longitud. Internamente en **metros**.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Length(f64);

impl Length {
    pub fn metres(value: f64) -> Result<Self> {
        finite(value, "longitud (m)").map(Self)
    }

    pub fn millimetres(value: f64) -> Result<Self> {
        finite(value, "longitud (mm)").map(|v| Self(v / 1000.0))
    }

    /// Longitud que debe ser estrictamente positiva (dimensiones de un espécimen).
    pub fn positive_millimetres(value: f64, field: &'static str) -> Result<Self> {
        positive(value, field).map(|v| Self(v / 1000.0))
    }

    pub fn millimetres_value(self) -> f64 {
        self.0 * 1000.0
    }

    pub fn metres_value(self) -> f64 {
        self.0
    }
}

magnitude!(Length, "longitud", "m");

// ------------------------------------------------------------------- masa ---

/// Masa. Internamente en **kilogramos**.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Mass(f64);

impl Mass {
    pub fn kilograms(value: f64) -> Result<Self> {
        finite(value, "masa (kg)").map(Self)
    }

    pub fn grams(value: f64) -> Result<Self> {
        finite(value, "masa (g)").map(|v| Self(v / 1000.0))
    }

    pub fn positive_grams(value: f64, field: &'static str) -> Result<Self> {
        positive(value, field).map(|v| Self(v / 1000.0))
    }

    pub fn grams_value(self) -> f64 {
        self.0 * 1000.0
    }
}

magnitude!(Mass, "masa", "kg");

// ------------------------------------------------------------------ fuerza ---

/// Fuerza. Internamente en **newtons**.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Force(f64);

impl Force {
    pub fn newtons(value: f64) -> Result<Self> {
        finite(value, "fuerza (N)").map(Self)
    }

    pub fn kilonewtons(value: f64) -> Result<Self> {
        finite(value, "fuerza (kN)").map(|v| Self(v * 1000.0))
    }

    pub fn positive_kilonewtons(value: f64, field: &'static str) -> Result<Self> {
        positive(value, field).map(|v| Self(v * 1000.0))
    }

    pub fn kilonewtons_value(self) -> f64 {
        self.0 / 1000.0
    }
}

magnitude!(Force, "fuerza", "N");

// -------------------------------------------------------------------- área ---

/// Área. Internamente en **metros cuadrados**.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Area(f64);

impl Area {
    pub fn square_metres(value: f64) -> Result<Self> {
        finite(value, "área (m²)").map(Self)
    }

    pub fn square_millimetres(value: f64) -> Result<Self> {
        finite(value, "área (mm²)").map(|v| Self(v / 1_000_000.0))
    }

    pub fn square_millimetres_value(self) -> f64 {
        self.0 * 1_000_000.0
    }

    /// Área de un círculo de diámetro `d`: A = π·d²/4.
    ///
    /// Es la definición geométrica del área, no un criterio normativo.
    pub fn of_circle(diameter: Length) -> Result<Self> {
        let d = diameter.0;
        if d <= 0.0 {
            return Err(Error::NotPositive {
                field: "diámetro",
                value: diameter.millimetres_value(),
            });
        }
        Ok(Self(std::f64::consts::PI * d * d / 4.0))
    }
}

magnitude!(Area, "área", "m²");

// ----------------------------------------------------------------- tensión ---

/// Tensión (esfuerzo). Internamente en **pascales**.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Stress(f64);

impl Stress {
    pub fn pascals(value: f64) -> Result<Self> {
        finite(value, "tensión (Pa)").map(Self)
    }

    pub fn megapascals(value: f64) -> Result<Self> {
        finite(value, "tensión (MPa)").map(|v| Self(v * 1_000_000.0))
    }

    pub fn megapascals_value(self) -> f64 {
        self.0 / 1_000_000.0
    }

    /// σ = F / A. Falla si el área no es positiva: nunca devuelve infinito.
    pub fn from_force_over_area(force: Force, area: Area) -> Result<Self> {
        if !area.0.is_finite() || area.0 <= 0.0 {
            return Err(Error::DivisionByZero {
                context: "tensión = fuerza / área (área no positiva)",
            });
        }
        Ok(Self(force.0 / area.0))
    }
}

magnitude!(Stress, "tensión", "Pa");

// ---------------------------------------------------------------- porcentaje ---

/// Porcentaje. Se guarda en la escala 0–100 (no como fracción).
///
/// No se impone rango: la humedad de un suelo puede superar el 100 % respecto al
/// peso seco, y poner un límite aquí inventaría una restricción que no existe.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Percent(f64);

impl Percent {
    pub fn of(value: f64) -> Result<Self> {
        finite(value, "porcentaje").map(Self)
    }
}

magnitude!(Percent, "porcentaje", "%");

// ------------------------------------------------------------ temperatura ---

/// Temperatura. Internamente en **grados Celsius**. Admite valores negativos.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Temperature(f64);

impl Temperature {
    pub fn celsius(value: f64) -> Result<Self> {
        finite(value, "temperatura (°C)").map(Self)
    }
}

magnitude!(Temperature, "temperatura", "°C");

// ------------------------------------------------------------------ unidad ---

/// Símbolos de unidad admitidos.
///
/// Existe para que una medición no pueda llevar una unidad inventada («mts», «kgs»,
/// «milímetros»): un texto libre como unidad es la puerta de entrada al error de
/// conversión que las magnitudes tipadas intentan evitar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Millimetre,
    Centimetre,
    Metre,
    Gram,
    Kilogram,
    Newton,
    Kilonewton,
    Megapascal,
    Percent,
    Celsius,
    Second,
}

impl Unit {
    pub const ALL: [Unit; 11] = [
        Unit::Millimetre,
        Unit::Centimetre,
        Unit::Metre,
        Unit::Gram,
        Unit::Kilogram,
        Unit::Newton,
        Unit::Kilonewton,
        Unit::Megapascal,
        Unit::Percent,
        Unit::Celsius,
        Unit::Second,
    ];

    /// Símbolo canónico de la unidad.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Millimetre => "mm",
            Self::Centimetre => "cm",
            Self::Metre => "m",
            Self::Gram => "g",
            Self::Kilogram => "kg",
            Self::Newton => "N",
            Self::Kilonewton => "kN",
            Self::Megapascal => "MPa",
            Self::Percent => "%",
            Self::Celsius => "°C",
            Self::Second => "s",
        }
    }

    /// Dimensiones que representa (`L`, `M`, `F`, `σ`, `%`, `T`, `t`).
    pub fn dimension(self) -> &'static str {
        match self {
            Self::Millimetre | Self::Centimetre | Self::Metre => "longitud",
            Self::Gram | Self::Kilogram => "masa",
            Self::Newton | Self::Kilonewton => "fuerza",
            Self::Megapascal => "tensión",
            Self::Percent => "porcentaje",
            Self::Celsius => "temperatura",
            Self::Second => "tiempo",
        }
    }

    /// Interpreta un símbolo. Rechaza cualquier cosa que no esté en la lista.
    pub fn parse(text: &str) -> Option<Self> {
        let normalized = text.trim();
        Self::ALL.into_iter().find(|unit| {
            unit.symbol() == normalized || unit.symbol().eq_ignore_ascii_case(normalized)
        })
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1.0)
    }

    #[test]
    fn longitud_ida_y_vuelta_sin_perdida() {
        let l = Length::millimetres(152.4).unwrap();
        assert!(close(l.metres_value(), 0.1524, 1e-12));
        assert!(close(l.millimetres_value(), 152.4, 1e-12));
    }

    #[test]
    fn una_longitud_no_admite_nan_ni_infinito() {
        assert!(matches!(
            Length::millimetres(f64::NAN),
            Err(Error::NonFinite { .. })
        ));
        assert!(matches!(
            Length::millimetres(f64::INFINITY),
            Err(Error::NonFinite { .. })
        ));
    }

    #[test]
    fn una_dimension_de_especimen_debe_ser_positiva() {
        assert!(Length::positive_millimetres(0.0, "diámetro").is_err());
        assert!(Length::positive_millimetres(-150.0, "diámetro").is_err());
        assert!(Length::positive_millimetres(150.0, "diámetro").is_ok());
    }

    #[test]
    fn area_de_circulo_de_150_mm() {
        // A = π·(0.15)²/4 = 0.0176714586764… m²  (17671.4586764… mm²)
        let a = Area::of_circle(Length::millimetres(150.0).unwrap()).unwrap();
        assert!(close(a.square_millimetres_value(), 17_671.458_676_44, 1e-9));
    }

    #[test]
    fn tension_de_500_kn_sobre_cilindro_de_150_mm() {
        // A = π·(0.15)²/4 = 0.0176714586764… m²
        // σ = 500 000 N / 0.0176714586764… m² = 28 294 212 Pa = 28.2942121… MPa
        let area = Area::of_circle(Length::millimetres(150.0).unwrap()).unwrap();
        let force = Force::kilonewtons(500.0).unwrap();
        let stress = Stress::from_force_over_area(force, area).unwrap();
        assert!(close(stress.megapascals_value(), 28.294_212_1, 1e-6));
    }

    #[test]
    fn tension_con_area_cero_falla_en_vez_de_devolver_infinito() {
        let area = Area::square_metres(0.0).unwrap();
        let force = Force::kilonewtons(500.0).unwrap();
        assert!(matches!(
            Stress::from_force_over_area(force, area),
            Err(Error::DivisionByZero { .. })
        ));
    }

    #[test]
    fn area_de_circulo_con_diametro_no_positivo_falla() {
        let cero = Length::millimetres(0.0).unwrap();
        assert!(Area::of_circle(cero).is_err());
    }

    #[test]
    fn fuerza_ida_y_vuelta_kn_n() {
        let f = Force::kilonewtons(12.5).unwrap();
        assert!(close(f.kilonewtons_value(), 12.5, 1e-12));
        assert!(close(f.canonical(), 12_500.0, 1e-12));
    }

    #[test]
    fn masa_ida_y_vuelta_g_kg() {
        let m = Mass::grams(1234.5).unwrap();
        assert!(close(m.grams_value(), 1234.5, 1e-12));
        assert!(close(m.canonical(), 1.2345, 1e-12));
    }

    #[test]
    fn porcentaje_admite_mas_de_cien_y_negativos() {
        assert!(Percent::of(115.0).is_ok());
        assert!(Percent::of(-3.0).is_ok());
        assert!(Percent::of(f64::NAN).is_err());
    }

    #[test]
    fn temperatura_admite_negativos() {
        assert!(Temperature::celsius(-5.0).is_ok());
    }

    #[test]
    fn display_lleva_unidad() {
        let l = Length::millimetres(150.0).unwrap();
        assert_eq!(l.to_string(), "0.15 m");
    }

    #[test]
    fn una_unidad_desconocida_no_se_acepta() {
        assert_eq!(Unit::parse("mm"), Some(Unit::Millimetre));
        assert_eq!(Unit::parse(" kN "), Some(Unit::Kilonewton));
        assert_eq!(Unit::parse("MPa"), Some(Unit::Megapascal));
        // Variantes que un usuario escribiría y que NO son símbolos válidos.
        assert_eq!(Unit::parse("mts"), None);
        assert_eq!(Unit::parse("kgs"), None);
        assert_eq!(Unit::parse("milímetros"), None);
        assert_eq!(Unit::parse(""), None);
    }

    #[test]
    fn cada_unidad_declara_su_dimension() {
        assert_eq!(Unit::Millimetre.dimension(), "longitud");
        assert_eq!(Unit::Kilogram.dimension(), "masa");
        assert_eq!(Unit::Kilonewton.dimension(), "fuerza");
        assert_eq!(Unit::Megapascal.dimension(), "tensión");
        for unit in Unit::ALL {
            assert!(!unit.symbol().is_empty());
            assert_eq!(Unit::parse(unit.symbol()), Some(unit));
        }
    }
}
