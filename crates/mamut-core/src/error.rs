//! Errores compartidos.
//!
//! Un solo tipo en la frontera evita que cada crate invente su vocabulario y que la
//! interfaz tenga que traducir diez enumeraciones distintas.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum Error {
    #[error("valor no finito en «{field}»")]
    NonFinite { field: &'static str },

    #[error("valor negativo no admitido en «{field}»: {value}")]
    Negative { field: &'static str, value: f64 },

    #[error("se requiere un valor mayor que cero en «{field}»: {value}")]
    NotPositive { field: &'static str, value: f64 },

    #[error("división por cero: {context}")]
    DivisionByZero { context: &'static str },

    #[error("identificador inválido: {0}")]
    InvalidId(String),

    #[error("fecha inválida: {0}")]
    InvalidDate(String),

    #[error("texto vacío en «{field}»")]
    Empty { field: &'static str },

    #[error("texto demasiado largo en «{field}»: {len} caracteres, máximo {max}")]
    TooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },

    #[error("transición de estado no permitida: {from} → {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },

    #[error("error de serialización: {0}")]
    Serialization(String),

    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_es_legible_para_un_tecnico() {
        let e = Error::NotPositive {
            field: "diámetro",
            value: 0.0,
        };
        assert_eq!(
            e.to_string(),
            "se requiere un valor mayor que cero en «diámetro»: 0"
        );
    }

    #[test]
    fn los_errores_se_pueden_comparar() {
        assert_eq!(Error::InvalidId("x".into()), Error::InvalidId("x".into()));
        assert_ne!(Error::InvalidId("x".into()), Error::InvalidId("y".into()));
    }
}
