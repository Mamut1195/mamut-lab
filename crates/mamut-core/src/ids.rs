//! Identificadores con tipo propio.
//!
//! Son **UUIDv7**: únicos entre instalaciones (preparado para una futura
//! sincronización, ADR-0003 §6) y ordenables por tiempo de creación. Que cada entidad
//! tenga su propio tipo impide pasar un `SampleId` donde se espera un `TestId`, un
//! error que el compilador debe atrapar y no una prueba en producción.

use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(uuid::Uuid);

        impl $name {
            /// Identificador nuevo (UUIDv7: ordenable por tiempo).
            pub fn new() -> Self {
                Self(uuid::Uuid::now_v7())
            }

            pub fn from_uuid(value: uuid::Uuid) -> Self {
                Self(value)
            }

            pub fn as_uuid(&self) -> uuid::Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = crate::error::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                uuid::Uuid::parse_str(s)
                    .map(Self)
                    .map_err(|_| crate::error::Error::InvalidId(s.to_owned()))
            }
        }
    };
}

id_newtype!(
    /// Muestra recibida en el laboratorio.
    SampleId
);
id_newtype!(
    /// Espécimen individual extraído o moldeado de una muestra.
    SpecimenId
);
id_newtype!(
    /// Ensayo practicado a un espécimen.
    TestId
);
id_newtype!(
    /// Medición individual capturada durante un ensayo.
    MeasurementId
);
id_newtype!(
    /// Resultado calculado (inmutable y versionado).
    ResultId
);
id_newtype!(
    /// Equipo de laboratorio.
    EquipmentId
);
id_newtype!(
    /// Entrada del registro de auditoría (solo inserción).
    AuditId
);
id_newtype!(
    /// Informe emitido.
    ReportId
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn dos_identificadores_nuevos_son_distintos() {
        assert_ne!(SampleId::new(), SampleId::new());
    }

    #[test]
    fn ida_y_vuelta_por_texto() {
        let id = TestId::new();
        let texto = id.to_string();
        let recuperado = TestId::from_str(&texto).unwrap();
        assert_eq!(id, recuperado);
    }

    #[test]
    fn un_texto_invalido_no_produce_identificador() {
        assert!(matches!(
            SampleId::from_str("no-es-un-uuid"),
            Err(crate::error::Error::InvalidId(_))
        ));
    }

    #[test]
    fn los_uuid_v7_crecen_con_el_tiempo() {
        // La propiedad que hace útil UUIDv7: dos ids seguidos se ordenan por creación.
        let a = SampleId::new();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = SampleId::new();
        assert!(a < b, "el UUIDv7 generado después debe ordenar después");
    }

    #[test]
    fn se_serializa_como_texto_plano() {
        let id = SampleId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        let vuelta: SampleId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, vuelta);
    }
}
