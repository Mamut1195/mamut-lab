//! Conformidad: el juicio que el producto **no** puede inventar.
//!
//! Regla del encargo: *«no presentar un resultado como conforme si no existe una
//! especificación aplicable»*. Aquí no se cumple por disciplina del programador, sino
//! porque [`Conformity::Conforms`] **exige** una [`SpecRef`] y unos [`SpecLimits`]: sin
//! ellos no hay forma de construir esa variante.
//!
//! Además, comparar contra el límite se hace con el valor **sin redondear**. Presentar
//! un valor redondeado a una décima justo en el límite y llamarlo «conforme» es un error
//! de ingeniería clásico (ADR-0006 §5).

use serde::{Deserialize, Serialize};

use crate::{NormRef, Result, StandardsError};

/// Referencia a la especificación **aplicable**: la del proyecto, del cliente o de la
/// norma de producto. No es la norma de ensayo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecRef {
    /// Documento del que sale el requisito (`NEC 2015`, `proyecto X`, `ASTM C94`).
    pub source: String,
    /// El requisito concreto dentro de ese documento.
    pub requirement: String,
}

impl SpecRef {
    pub fn new(source: impl Into<String>, requirement: impl Into<String>) -> Result<Self> {
        let source = source.into().trim().to_owned();
        let requirement = requirement.into().trim().to_owned();
        if source.is_empty() {
            return Err(StandardsError::Empty {
                field: "spec.source",
            });
        }
        if requirement.is_empty() {
            return Err(StandardsError::Empty {
                field: "spec.requirement",
            });
        }
        Ok(Self {
            source,
            requirement,
        })
    }
}

/// Límites numéricos de aceptación, en la unidad del resultado.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpecLimits {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
}

impl SpecLimits {
    pub fn new(min: Option<f64>, max: Option<f64>) -> Result<Self> {
        for value in [min, max].into_iter().flatten() {
            if !value.is_finite() {
                return Err(StandardsError::InvalidLimits { min, max });
            }
        }
        if let (Some(lo), Some(hi)) = (min, max)
            && lo > hi
        {
            return Err(StandardsError::InvalidLimits { min, max });
        }
        Ok(Self { min, max })
    }

    /// Si el valor cae dentro del intervalo. Los límites son **inclusivos**.
    pub fn accepts(&self, value: f64) -> bool {
        self.min.is_none_or(|lo| value >= lo) && self.max.is_none_or(|hi| value <= hi)
    }

    /// Unos límites sin ningún extremo no limitan nada: no sirven como especificación.
    pub fn is_empty(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }
}

/// Por qué un resultado no cumple.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum FailureReason {
    BelowMinimum { limit: f64, actual: f64 },
    AboveMaximum { limit: f64, actual: f64 },
}

/// Por qué **no se evaluó** la conformidad. Nunca se confunde con «no cumple».
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotEvaluatedReason {
    /// No hay especificación aplicable registrada para este material o proyecto.
    NoApplicableSpec,
    /// La referencia normativa no ha sido revisada por un ingeniero.
    NormNotVerified,
    /// El resultado no es un número finito.
    ValueNotFinite,
    /// La especificación no fija ningún extremo.
    EmptyLimits,
}

impl NotEvaluatedReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoApplicableSpec => {
                "sin especificación aplicable: no se emite juicio de conformidad"
            }
            Self::NormNotVerified => {
                "referencia normativa pendiente de revisión: no se emite juicio de conformidad"
            }
            Self::ValueNotFinite => "el resultado no es un valor numérico válido",
            Self::EmptyLimits => "la especificación no fija límites",
        }
    }
}

/// Resultado del juicio de conformidad.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Conformity {
    Conforms {
        spec: SpecRef,
        limits: SpecLimits,
    },
    DoesNotConform {
        spec: SpecRef,
        limits: SpecLimits,
        reason: FailureReason,
    },
    NotEvaluated {
        reason: NotEvaluatedReason,
    },
}

impl Conformity {
    /// Evalúa `actual` (valor **sin redondear**) contra la norma y la especificación.
    ///
    /// El orden de las comprobaciones es deliberado: primero se descarta un valor no
    /// numérico, después la falta de revisión normativa, y solo al final la falta de
    /// especificación. Cualquiera de las tres produce `NotEvaluated`, nunca un veredicto.
    pub fn evaluate(
        actual: f64,
        norm: &NormRef,
        spec: Option<&SpecRef>,
        limits: Option<&SpecLimits>,
    ) -> Self {
        if !actual.is_finite() {
            return Self::NotEvaluated {
                reason: NotEvaluatedReason::ValueNotFinite,
            };
        }
        if !norm.is_verified() {
            return Self::NotEvaluated {
                reason: NotEvaluatedReason::NormNotVerified,
            };
        }
        let (Some(spec), Some(limits)) = (spec, limits) else {
            return Self::NotEvaluated {
                reason: NotEvaluatedReason::NoApplicableSpec,
            };
        };
        if limits.is_empty() {
            return Self::NotEvaluated {
                reason: NotEvaluatedReason::EmptyLimits,
            };
        }
        if limits.accepts(actual) {
            return Self::Conforms {
                spec: spec.clone(),
                limits: *limits,
            };
        }
        let reason = match (limits.min, limits.max) {
            (Some(lo), _) if actual < lo => FailureReason::BelowMinimum { limit: lo, actual },
            (_, Some(hi)) => FailureReason::AboveMaximum { limit: hi, actual },
            _ => FailureReason::BelowMinimum {
                limit: f64::NAN,
                actual,
            },
        };
        Self::DoesNotConform {
            spec: spec.clone(),
            limits: *limits,
            reason,
        }
    }

    /// `Some(true)` conforme, `Some(false)` no conforme, `None` **no evaluado**.
    ///
    /// Tener tres estados (y no un `bool`) es lo que impide tratar «no evaluado» como
    /// «conforme».
    pub fn verdict(&self) -> Option<bool> {
        match self {
            Self::Conforms { .. } => Some(true),
            Self::DoesNotConform { .. } => Some(false),
            Self::NotEvaluated { .. } => None,
        }
    }

    pub fn is_evaluated(&self) -> bool {
        self.verdict().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NormBody, NormStatus};
    use mamut_core::CivilDate;

    fn norm(status: NormStatus) -> NormRef {
        NormRef::new(NormBody::Astm, "C39/C39M", 2026, "Compresión", status).unwrap()
    }

    fn verified() -> NormRef {
        norm(NormStatus::Verified {
            reviewed_by: "Ing. Ana Torres".into(),
            reviewed_on: CivilDate::new(2026, 11, 20).unwrap(),
        })
    }

    fn limits(min: f64) -> SpecLimits {
        SpecLimits::new(Some(min), None).unwrap()
    }

    fn spec() -> SpecRef {
        SpecRef::new("NEC 2015", "f'c de diseño").unwrap()
    }

    #[test]
    fn sin_norma_revisada_no_hay_veredicto_aunque_haya_limites() {
        let c = Conformity::evaluate(
            30.0,
            &norm(NormStatus::PendingReview),
            Some(&spec()),
            Some(&limits(21.0)),
        );
        assert_eq!(
            c,
            Conformity::NotEvaluated {
                reason: NotEvaluatedReason::NormNotVerified
            }
        );
        assert_eq!(
            c.verdict(),
            None,
            "no evaluado no es ni conforme ni no conforme"
        );
    }

    #[test]
    fn sin_especificacion_no_hay_veredicto_aunque_la_norma_este_revisada() {
        let c = Conformity::evaluate(30.0, &verified(), None, Some(&limits(21.0)));
        assert_eq!(
            c,
            Conformity::NotEvaluated {
                reason: NotEvaluatedReason::NoApplicableSpec
            }
        );
    }

    #[test]
    fn con_limites_vacios_no_hay_veredicto() {
        let vacios = SpecLimits::new(None, None).unwrap();
        let c = Conformity::evaluate(30.0, &verified(), Some(&spec()), Some(&vacios));
        assert_eq!(
            c,
            Conformity::NotEvaluated {
                reason: NotEvaluatedReason::EmptyLimits
            }
        );
    }

    #[test]
    fn un_valor_no_finito_no_se_evalua() {
        let c = Conformity::evaluate(f64::NAN, &verified(), Some(&spec()), Some(&limits(21.0)));
        assert_eq!(
            c,
            Conformity::NotEvaluated {
                reason: NotEvaluatedReason::ValueNotFinite
            }
        );
    }

    #[test]
    fn conforme_cuando_alcanza_el_minimo_inclusive_el_limite() {
        let c = Conformity::evaluate(21.0, &verified(), Some(&spec()), Some(&limits(21.0)));
        assert_eq!(c.verdict(), Some(true));
    }

    #[test]
    fn no_conforme_por_debajo_del_minimo() {
        let c = Conformity::evaluate(20.9, &verified(), Some(&spec()), Some(&limits(21.0)));
        assert_eq!(c.verdict(), Some(false));
        match c {
            Conformity::DoesNotConform { reason, .. } => assert_eq!(
                reason,
                FailureReason::BelowMinimum {
                    limit: 21.0,
                    actual: 20.9
                }
            ),
            other => panic!("se esperaba no conformidad, llegó {other:?}"),
        }
    }

    #[test]
    fn no_conforme_por_encima_del_maximo() {
        let max = SpecLimits::new(None, Some(35.0)).unwrap();
        let c = Conformity::evaluate(35.1, &verified(), Some(&spec()), Some(&max));
        assert_eq!(c.verdict(), Some(false));
    }

    #[test]
    fn los_limites_invertidos_no_se_aceptan() {
        assert!(SpecLimits::new(Some(40.0), Some(20.0)).is_err());
    }

    #[test]
    fn los_limites_no_finitos_no_se_aceptan() {
        assert!(SpecLimits::new(Some(f64::NAN), None).is_err());
        assert!(SpecLimits::new(None, Some(f64::INFINITY)).is_err());
    }

    #[test]
    fn una_especificacion_sin_origen_no_es_especificacion() {
        assert!(SpecRef::new("  ", "f'c").is_err());
        assert!(SpecRef::new("NEC", "  ").is_err());
    }
}
