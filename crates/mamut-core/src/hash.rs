//! Huellas de integridad.
//!
//! Un resultado calculado debe poder demostrar **con qué entradas** se calculó. Se
//! guarda un SHA-256 del *snapshot* canónico de las entradas; si alguien edita los
//! datos sin recalcular, la huella deja de coincidir y la aplicación lo detecta
//! (ADR-0006 §3).

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// SHA-256 en hexadecimal minúsculas de unos bytes cualesquiera.
pub fn hex_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Huella de una estructura serializable como JSON.
///
/// La estabilidad depende del orden de los campos, que en `serde` es el de la
/// declaración del `struct`. Por eso las entradas de cálculo deben ser estructuras
/// con campos ordenados y sin mapas de orden no determinista (`HashMap`).
pub fn fingerprint<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::Serialization(e.to_string()))?;
    Ok(hex_digest(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Entrada {
        diametro_mm: f64,
        altura_mm: f64,
        carga_kn: f64,
    }

    #[test]
    fn la_misma_entrada_produce_la_misma_huella() {
        let a = Entrada {
            diametro_mm: 150.0,
            altura_mm: 300.0,
            carga_kn: 500.0,
        };
        assert_eq!(fingerprint(&a).unwrap(), fingerprint(&a).unwrap());
    }

    #[test]
    fn cambiar_un_campo_cambia_la_huella() {
        let a = Entrada {
            diametro_mm: 150.0,
            altura_mm: 300.0,
            carga_kn: 500.0,
        };
        let b = Entrada {
            diametro_mm: 150.0,
            altura_mm: 300.0,
            carga_kn: 500.1,
        };
        assert_ne!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
    }

    #[test]
    fn la_huella_es_hexadecimal_de_64_caracteres() {
        let h = fingerprint("hola").unwrap();
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sha256_conocido_de_cadena_vacia() {
        // Valor de referencia público de SHA-256 para la entrada vacía.
        assert_eq!(
            hex_digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
