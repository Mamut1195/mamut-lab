//! `mamut-domain` — entidades e invariantes del laboratorio.
//!
//! Aquí viven las reglas que no dependen de la base de datos ni de la pantalla:
//! qué es una muestra válida, cuándo un ensayo puede pasar de registrado a calculado,
//! y por qué un dato original no se puede reescribir.
//!
//! Este crate **no hace I/O** y no conoce Rune (ADR-0003).

pub mod measurement;
pub mod sample;
pub mod specimen;
pub mod test_record;

pub use measurement::Measurement;
pub use sample::{Material, Sample, SampleCode, SampleState};
pub use specimen::Specimen;
pub use test_record::{ResultRef, TestRecord, TestState};

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum DomainError {
    #[error(transparent)]
    Core(#[from] mamut_core::Error),

    #[error(transparent)]
    Standards(#[from] mamut_standards::StandardsError),

    #[error("transición no permitida: {from} → {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },

    #[error("no se puede informar un ensayo sin resultado calculado")]
    MissingResult,

    #[error("una corrección exige un motivo")]
    MissingCorrectionReason,
}

pub type Result<T> = std::result::Result<T, DomainError>;
