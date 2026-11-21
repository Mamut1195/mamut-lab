//! Medición: el dato original, tal como se tecleó.
//!
//! Regla 4 del encargo: *conservar los datos originales*. Por eso se guarda el texto
//! que introdujo el operador, con su unidad, y una corrección **no modifica** la fila
//! anterior: crea una nueva versión que la sucede y declara el motivo.

use serde::{Deserialize, Serialize};

use mamut_core::{Error as CoreError, MeasurementId, Timestamp, Unit};

use crate::{DomainError, Result};

/// Un valor medido debe ser un número **finito**. `parse::<f64>()` acepta `NaN`, `inf` y
/// `1e999` (este último como infinito), y ninguno de los tres es una medición: dejarlos
/// pasar propagaría un NaN hasta el cálculo y de ahí al informe.
fn finite_number(text: &str) -> Result<f64> {
    match text.parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(value),
        _ => Err(CoreError::Message(format!("«{text}» no es un valor numérico finito")).into()),
    }
}

/// Una lectura capturada durante un ensayo.
///
/// `#[non_exhaustive]`: fuera de este crate **no se puede construir con un literal de
/// estructura**, así que la única puerta de entrada es [`Measurement::new`], que valida
/// número finito y unidad admitida. Sin esto, `Measurement { unit: "mts", .. }` colaría
/// una unidad inventada saltándose el constructor (hallazgo de la revisión adversarial).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub id: MeasurementId,
    /// Variable medida: `diametro`, `altura`, `masa`, `carga_maxima`, `humedad`…
    pub variable: String,
    /// Valor original como texto decimal. No se convierte a `f64` al guardar.
    pub value_text: String,
    /// Unidad explícita: `mm`, `g`, `kN`, `%`.
    pub unit: String,
    /// Número de versión: 1 es la lectura original.
    pub revision: u32,
    /// Revisión a la que sustituye, si es una corrección.
    pub supersedes: Option<u32>,
    /// Motivo de la corrección. Obligatorio cuando `supersedes` existe.
    pub correction_reason: Option<String>,
    pub recorded_at: Timestamp,
}

impl Measurement {
    pub fn new(
        variable: impl Into<String>,
        value_text: impl Into<String>,
        unit: impl Into<String>,
        recorded_at: Timestamp,
    ) -> Result<Self> {
        let variable = variable.into().trim().to_lowercase();
        if variable.is_empty() {
            return Err(CoreError::Empty { field: "variable" }.into());
        }
        let value_text = value_text.into().trim().to_owned();
        if value_text.is_empty() {
            return Err(CoreError::Empty {
                field: "valor medido",
            }
            .into());
        }
        // El valor debe ser un número **finito**: se guarda como texto, pero un texto que
        // no es un número (o que es NaN/infinito) no es una medición.
        finite_number(&value_text)?;
        let raw_unit = unit.into().trim().to_owned();
        if raw_unit.is_empty() {
            return Err(CoreError::Empty { field: "unidad" }.into());
        }
        // Se guarda el símbolo canónico, no lo que tecleó el usuario: así el dominio y la
        // base coinciden («MM» y «mm» son la misma unidad, y la base solo admite «mm»).
        let unit = Unit::parse(&raw_unit)
            .ok_or_else(|| {
                CoreError::Message(format!(
                    "unidad desconocida: «{raw_unit}» (se admite mm, cm, m, g, kg, N, kN, MPa, %, °C, s)"
                ))
            })?
            .symbol()
            .to_owned();
        Ok(Self {
            id: MeasurementId::new(),
            variable,
            value_text,
            unit,
            revision: 1,
            supersedes: None,
            correction_reason: None,
            recorded_at,
        })
    }

    /// Reconstruye una medición ya guardada.
    ///
    /// Es la vía que usa la persistencia al leer: pasa por la misma validación que
    /// [`Measurement::new`], así que una fila corrupta no circula como medición válida.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: MeasurementId,
        variable: String,
        value_text: String,
        unit: String,
        revision: u32,
        supersedes: Option<u32>,
        correction_reason: Option<String>,
        recorded_at: Timestamp,
    ) -> Result<Self> {
        let measurement = Self {
            id,
            variable: variable.trim().to_owned(),
            value_text: value_text.trim().to_owned(),
            // Normaliza igual que `new`: una base migrada puede traer «MM» o «KGS».
            unit: Unit::parse(unit.trim())
                .map(|unit| unit.symbol().to_owned())
                .unwrap_or_else(|| unit.trim().to_owned()),
            revision,
            supersedes,
            correction_reason,
            recorded_at,
        };
        measurement.validate()?;
        Ok(measurement)
    }

    /// Crea la versión corregida. **No** modifica `self`: devuelve una medición nueva.
    pub fn corrected(
        &self,
        new_value_text: impl Into<String>,
        reason: impl Into<String>,
        recorded_at: Timestamp,
    ) -> Result<Self> {
        let reason = reason.into().trim().to_owned();
        if reason.is_empty() {
            return Err(DomainError::MissingCorrectionReason);
        }
        let new_value_text = new_value_text.into().trim().to_owned();
        if new_value_text.is_empty() {
            return Err(CoreError::Empty {
                field: "valor medido",
            }
            .into());
        }
        finite_number(&new_value_text)?;
        Ok(Self {
            id: MeasurementId::new(),
            variable: self.variable.clone(),
            value_text: new_value_text,
            unit: self.unit.clone(),
            revision: self.revision + 1,
            supersedes: Some(self.revision),
            correction_reason: Some(reason),
            recorded_at,
        })
    }

    /// Valor como `f64`, para el cálculo. El texto original se conserva intacto.
    ///
    /// Devuelve solo números finitos: una medición nunca puede aportar `NaN` a un cálculo.
    pub fn parse_value(&self) -> Result<f64> {
        finite_number(&self.value_text)
    }

    pub fn is_correction(&self) -> bool {
        self.supersedes.is_some()
    }

    /// Revalida los invariantes del dato.
    ///
    /// Se usa al **leer de la base**: una fila escrita por una versión anterior o
    /// manipulada por fuera no debe circular como medición válida.
    pub fn validate(&self) -> Result<()> {
        if self.variable.trim().is_empty() {
            return Err(CoreError::Empty { field: "variable" }.into());
        }
        finite_number(&self.value_text)?;
        if Unit::parse(&self.unit).is_none() {
            return Err(CoreError::Message(format!(
                "unidad desconocida en la medición: «{}»",
                self.unit
            ))
            .into());
        }
        if self.revision == 0 {
            return Err(CoreError::Message("la revisión debe empezar en 1".into()).into());
        }
        if self.supersedes.is_some() && self.correction_reason.is_none() {
            return Err(DomainError::MissingCorrectionReason);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> Timestamp {
        Timestamp::from_millis(1_700_000_000_000)
    }

    #[test]
    fn una_medicion_guarda_el_texto_tal_cual() {
        let m = Measurement::new("Diametro", " 150.0 ", "mm", ts()).unwrap();
        assert_eq!(m.variable, "diametro", "la variable se normaliza");
        assert_eq!(
            m.value_text, "150.0",
            "el valor se recorta, no se reformatea"
        );
        assert_eq!(m.unit, "mm");
        assert_eq!(m.revision, 1);
        assert!(!m.is_correction());
    }

    #[test]
    fn una_medicion_exige_variable_valor_y_unidad() {
        assert!(Measurement::new("", "1", "mm", ts()).is_err());
        assert!(Measurement::new("masa", "", "g", ts()).is_err());
        assert!(Measurement::new("masa", "1", "", ts()).is_err());
    }

    #[test]
    fn un_valor_no_numerico_no_es_una_medicion() {
        assert!(Measurement::new("masa", "doce", "g", ts()).is_err());
        assert!(Measurement::new("masa", "12,300.5", "g", ts()).is_err());
    }

    #[test]
    fn nan_e_infinito_no_son_mediciones() {
        // `parse::<f64>()` acepta estos textos; una medición no puede aceptarlos.
        assert!(Measurement::new("masa", "NaN", "g", ts()).is_err());
        assert!(Measurement::new("masa", "inf", "g", ts()).is_err());
        assert!(Measurement::new("masa", "-inf", "g", ts()).is_err());
        assert!(Measurement::new("masa", "1e999", "g", ts()).is_err());
        // Y una corrección tampoco puede introducirlos.
        let original = Measurement::new("masa", "1230.0", "g", ts()).unwrap();
        assert!(original.corrected("NaN", "motivo", ts()).is_err());
    }

    #[test]
    fn una_unidad_inventada_no_se_acepta() {
        assert!(Measurement::new("masa", "1", "mts", ts()).is_err());
        assert!(Measurement::new("masa", "1", "milímetros", ts()).is_err());
        for unit in ["mm", "cm", "m", "g", "kg", "N", "kN", "MPa", "%", "°C", "s"] {
            assert!(
                Measurement::new("masa", "1", unit, ts()).is_ok(),
                "la unidad {unit} debería admitirse"
            );
        }
    }

    #[test]
    fn la_unidad_se_guarda_en_su_simbolo_canonico() {
        // El dominio admite el **símbolo** en cualquier caja («KG» y «kg» son lo mismo) y
        // guarda siempre la forma canónica, para coincidir con lo que admite la base.
        // Las abreviaturas que no son símbolo («KGS», «mts») no se aceptan al crear; la
        // migración V004 las repara en bases antiguas.
        let m = Measurement::new("masa", "1.5", "KG", ts()).unwrap();
        assert_eq!(m.unit, "kg");
        let m = Measurement::new("carga_maxima", "500", "kn", ts()).unwrap();
        assert_eq!(m.unit, "kN");
        assert!(Measurement::new("masa", "1.5", "KGS", ts()).is_err());
    }

    #[test]
    fn la_correccion_no_modifica_el_original() {
        let original = Measurement::new("masa", "1230.0", "g", ts()).unwrap();
        let corregida = original
            .corrected("1230.5", "lectura mal transcrita", ts())
            .unwrap();
        assert_eq!(original.value_text, "1230.0", "el original queda intacto");
        assert_eq!(original.revision, 1);
        assert_eq!(corregida.value_text, "1230.5");
        assert_eq!(corregida.revision, 2);
        assert_eq!(corregida.supersedes, Some(1));
        assert_eq!(
            corregida.correction_reason.as_deref(),
            Some("lectura mal transcrita")
        );
        assert_ne!(
            original.id, corregida.id,
            "cada versión es una fila distinta"
        );
    }

    #[test]
    fn una_correccion_exige_motivo() {
        let m = Measurement::new("masa", "1230.0", "g", ts()).unwrap();
        assert_eq!(
            m.corrected("1230.5", "   ", ts()),
            Err(DomainError::MissingCorrectionReason)
        );
    }

    #[test]
    fn la_correccion_conserva_variable_y_unidad() {
        let m = Measurement::new("carga_maxima", "500", "kN", ts()).unwrap();
        let c = m
            .corrected("505", "recalibración del equipo", ts())
            .unwrap();
        assert_eq!(c.variable, "carga_maxima");
        assert_eq!(c.unit, "kN");
    }

    #[test]
    fn el_valor_se_parsea_para_calcular_sin_perder_el_texto() {
        let m = Measurement::new("masa", "1230.50", "g", ts()).unwrap();
        assert!((m.parse_value().unwrap() - 1230.50).abs() < 1e-9);
        assert_eq!(m.value_text, "1230.50");
    }
}
