//! Descriptor de ensayo: la ficha que dice qué se hace, con qué método y con qué norma.

use serde::{Deserialize, Serialize};

use crate::{NormRef, Result, StandardsError};

/// Si el procedimiento es normativo o interno.
///
/// La distinción es obligatoria en el informe (regla 3 del encargo). Un método interno
/// nunca debe presentarse como normativo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MethodKind {
    #[serde(rename = "Normative")]
    Normative,
    #[serde(rename = "Internal")]
    Internal,
}

impl MethodKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normative => "Normativo",
            Self::Internal => "Método interno",
        }
    }
}

/// Código estable de un tipo de ensayo (`CONC-COMP-CYL`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TestCode(String);

impl TestCode {
    pub const MAX_LEN: usize = 32;

    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let normalized = raw.into().trim().to_ascii_uppercase();
        if normalized.is_empty() {
            return Err(StandardsError::Empty { field: "test.code" });
        }
        if normalized.len() > Self::MAX_LEN {
            return Err(StandardsError::TooLong {
                field: "test.code",
                len: normalized.len(),
                max: Self::MAX_LEN,
            });
        }
        if !normalized
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(StandardsError::InvalidTestCode(normalized));
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TestCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for TestCode {
    type Error = StandardsError;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<TestCode> for String {
    fn from(value: TestCode) -> Self {
        value.0
    }
}

/// Todo lo que identifica un ensayo antes de ejecutarlo.
#[derive(Clone, Debug, PartialEq)]
pub struct TestDescriptor {
    pub code: TestCode,
    pub name: String,
    pub method_kind: MethodKind,
    pub norm: Option<NormRef>,
    /// Solo puede ser `true` si hay norma revisada (lo garantiza [`TestDescriptor::new`]).
    pub conformity_enabled: bool,
}

impl TestDescriptor {
    pub fn new(
        code: TestCode,
        name: impl Into<String>,
        method_kind: MethodKind,
        norm: Option<NormRef>,
        conformity_enabled: bool,
    ) -> Result<Self> {
        let name = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(StandardsError::Empty { field: "test.name" });
        }
        if conformity_enabled && !norm.as_ref().is_some_and(NormRef::is_verified) {
            return Err(StandardsError::ConformityWithoutVerifiedNorm);
        }
        Ok(Self {
            code,
            name,
            method_kind,
            norm,
            conformity_enabled,
        })
    }

    /// Si este ensayo puede emitir un veredicto hoy.
    pub fn can_emit_verdict(&self) -> bool {
        self.conformity_enabled && self.norm.as_ref().is_some_and(NormRef::is_verified)
    }

    /// Cita para el informe, o el aviso de que no hay norma asociada.
    pub fn citation(&self) -> Option<String> {
        self.norm.as_ref().map(NormRef::citation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NormBody, NormStatus};
    use mamut_core::CivilDate;

    fn code() -> TestCode {
        TestCode::new("CONC-COMP-CYL").unwrap()
    }

    fn verified_norm() -> NormRef {
        NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2026,
            "Compresión de cilindros",
            NormStatus::Verified {
                reviewed_by: "Ing. Ana Torres".into(),
                reviewed_on: CivilDate::new(2026, 11, 20).unwrap(),
            },
        )
        .unwrap()
    }

    fn pending_norm() -> NormRef {
        NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2026,
            "Compresión de cilindros",
            NormStatus::PendingReview,
        )
        .unwrap()
    }

    #[test]
    fn un_codigo_se_normaliza_a_mayusculas() {
        assert_eq!(
            TestCode::new("  conc-comp-cyl ").unwrap().as_str(),
            "CONC-COMP-CYL"
        );
    }

    #[test]
    fn un_codigo_con_caracteres_raros_se_rechaza() {
        assert!(TestCode::new("CONC COMP").is_err());
        assert!(TestCode::new("CONC_COMP").is_err());
        assert!(TestCode::new("ñ").is_err());
        assert!(TestCode::new("").is_err());
    }

    #[test]
    fn un_codigo_demasiado_largo_se_rechaza() {
        let largo = "A".repeat(TestCode::MAX_LEN + 1);
        assert!(TestCode::new(largo).is_err());
    }

    #[test]
    fn no_se_puede_habilitar_conformidad_sin_norma() {
        assert_eq!(
            TestDescriptor::new(code(), "Compresión", MethodKind::Normative, None, true),
            Err(StandardsError::ConformityWithoutVerifiedNorm)
        );
    }

    #[test]
    fn no_se_puede_habilitar_conformidad_con_norma_sin_revisar() {
        assert_eq!(
            TestDescriptor::new(
                code(),
                "Compresión",
                MethodKind::Normative,
                Some(pending_norm()),
                true
            ),
            Err(StandardsError::ConformityWithoutVerifiedNorm)
        );
    }

    #[test]
    fn con_norma_revisada_si_se_puede_y_la_cita_aparece() {
        let d = TestDescriptor::new(
            code(),
            "Compresión",
            MethodKind::Normative,
            Some(verified_norm()),
            true,
        )
        .unwrap();
        assert!(d.can_emit_verdict());
        assert_eq!(d.citation().as_deref(), Some("ASTM C39/C39M-26"));
    }

    #[test]
    fn un_metodo_interno_sin_norma_es_valido_pero_no_dictamina() {
        let d =
            TestDescriptor::new(code(), "Compresión", MethodKind::Internal, None, false).unwrap();
        assert!(!d.can_emit_verdict());
        assert_eq!(d.citation(), None);
        assert_eq!(d.method_kind.label(), "Método interno");
    }

    #[test]
    fn un_descriptor_sin_nombre_no_existe() {
        assert!(TestDescriptor::new(code(), "   ", MethodKind::Internal, None, false).is_err());
    }

    #[test]
    fn el_codigo_va_y_vuelve_por_json() {
        let c = TestCode::new("SOIL-GRAN").unwrap();
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"SOIL-GRAN\"");
        let vuelta: TestCode = serde_json::from_str(&json).unwrap();
        assert_eq!(c, vuelta);
    }

    #[test]
    fn un_codigo_invalido_no_entra_por_json() {
        assert!(serde_json::from_str::<TestCode>("\"con espacios\"").is_err());
    }
}
