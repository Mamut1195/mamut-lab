//! `mamut-calculations` — algoritmos puros.
//!
//! **Nada de I/O, nada de interfaz, nada de base de datos.** Entra una estructura con
//! magnitudes tipadas y sale otra con los resultados. Algunos llevan además avisos de
//! plausibilidad ([`FieldDensity::warnings`]) o datos del ajuste que los sostiene
//! ([`LiquidLimitFit`], `CurvePeak::points_used`): un número sin su contexto no se puede
//! auditar. Así se puede probar un cálculo en milisegundos y razonar sobre él sin abrir una
//! ventana.
//!
//! **Frontera deliberada.** Este crate calcula *magnitudes derivadas* (área, tensión,
//! densidad, esbeltez). No decide si un resultado es aceptable: eso es conformidad, y
//! vive en `mamut-standards` con la puerta de la norma revisada (ADR-0006).

pub mod atterberg;
pub mod cbr;
pub mod compaction;
pub mod cylinder;
pub mod field_density;
pub mod granulometry;
pub mod rounding;

pub use atterberg::{
    LimitPoint, LiquidLimitFit, blows_within_typical_range, liquid_limit_multi_point,
    liquid_limit_one_point, moisture_percent, plastic_limit, plasticity_index,
};
pub use cbr::{
    CbrResult, MAX_PLAUSIBLE_CBR, PenetrationPoint, STANDARD_LOAD_01_MPA, STANDARD_LOAD_02_MPA,
    cbr_from_curve, cbr_percent, tangent_origin_offset,
};
pub use compaction::{CompactionPoint, CurvePeak, curve_peak, dry_density, wet_density};
pub use cylinder::{CompressionOutcome, CylinderCompressionInput, ENGINE_VERSION, calculate};
pub use field_density::{FieldDensity, field_density};
pub use granulometry::{GranulometryOutcome, SievePass, sieve_analysis};
pub use rounding::{RoundingPolicy, round_half_away_from_zero};

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum CalcError {
    #[error(transparent)]
    Core(#[from] mamut_core::Error),

    #[error("el cálculo requiere una referencia normativa, aunque no esté revisada")]
    MissingNorm,

    #[error("la entrada no es coherente: {0}")]
    Incoherent(String),
}

pub type Result<T> = std::result::Result<T, CalcError>;
