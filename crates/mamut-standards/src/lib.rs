//! `mamut-standards` — referencias normativas y juicio de conformidad.
//!
//! Este crate implementa la regla más importante del producto (ADR-0006, ADR-0009):
//!
//! > **No se emite un juicio de conformidad si no existe una especificación aplicable
//! > y la referencia normativa no ha sido revisada por un ingeniero.**
//!
//! No es una comprobación que el programador deba recordar: el tipo [`Conformity`] no
//! tiene forma de construirse en estado «conforme» sin una [`SpecRef`] y unos
//! [`SpecLimits`]. Si se intenta, el resultado es [`Conformity::NotEvaluated`].
//!
//! Las normas son **datos** ([`pack`]), no código: se cargan al final sin recompilar.

pub mod conformity;
pub mod descriptor;
pub mod norm;
pub mod pack;

pub use conformity::{Conformity, FailureReason, NotEvaluatedReason, SpecLimits, SpecRef};
pub use descriptor::{MethodKind, TestCode, TestDescriptor};
pub use norm::{NormBody, NormRef, NormStatus};
pub use pack::{Confidence, LoadedPack, NormPack, PackIssue, PackSeverity, PackStatus};

/// Errores del registro normativo.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum StandardsError {
    #[error("texto vacío en «{field}»")]
    Empty { field: &'static str },

    #[error("texto demasiado largo en «{field}»: {len} caracteres, máximo {max}")]
    TooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },

    #[error("código de ensayo inválido: «{0}» (se admite A-Z, 0-9 y guion)")]
    InvalidTestCode(String),

    #[error("un ensayo no puede evaluar conformidad sin una norma revisada")]
    ConformityWithoutVerifiedNorm,

    #[error("año normativo implausible: {year}")]
    ImplausibleYear { year: u16 },

    #[error("límites de especificación inválidos: mínimo {min:?} mayor que máximo {max:?}")]
    InvalidLimits { min: Option<f64>, max: Option<f64> },

    #[error("una norma marcada como revisada debe indicar revisor y fecha")]
    VerifiedWithoutReview,

    #[error("norma desconocida: {0}")]
    UnknownNorm(String),

    #[error("ensayo desconocido: {0}")]
    UnknownTest(String),

    #[error("el pack declara la versión {found}; esta aplicación admite hasta la {max}")]
    UnsupportedPackVersion { found: u32, max: u32 },
}

pub type Result<T> = std::result::Result<T, StandardsError>;

/// Última versión de *norm pack* que esta aplicación sabe leer.
///
/// Se sube solo cuando el formato cambia de forma incompatible. Un pack más nuevo se
/// rechaza en lugar de interpretarse a medias (regla 6 de `docs/normas/README.md`).
pub const MAX_PACK_VERSION: u32 = 1;

/// Primer año que se considera una edición normativa plausible.
pub const MIN_PLAUSIBLE_YEAR: u16 = 1900;

/// Último año admitido. Es una cota de cordura contra errores de tecleo, no una
/// comprobación de vigencia: que una edición siga siendo la actual lo determina la
/// revisión de metadatos, no una comparación con el reloj.
pub const MAX_PLAUSIBLE_YEAR: u16 = 2100;
