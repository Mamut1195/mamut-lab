//! `mamut-core` — tipos base del producto.
//!
//! Contiene lo que todos los demás crates necesitan y nada más: identificadores,
//! tiempo inyectable, unidades con tipos propios, errores y huellas de integridad.
//!
//! **Reglas de este crate**
//! - No hace I/O de negocio. `SystemClock` es la única lectura del reloj del sistema,
//!   y está detrás del trait [`Clock`] para que el dominio reciba el tiempo inyectado.
//! - No conoce la base de datos ni la interfaz.
//! - Ninguna magnitud física viaja como `f64` suelto: cada dimensión tiene su tipo
//!   ([`Length`], [`Force`], [`Stress`], …), y convertir unidades es siempre explícito.

pub mod audit;
pub mod error;
pub mod hash;
pub mod ids;
pub mod time;
pub mod units;

pub use audit::{Actor, AuditAction, AuditEntry};
pub use error::{Error, Result};
pub use ids::{
    AuditId, EquipmentId, MeasurementId, ReportId, ResultId, SampleId, SpecimenId, TestId,
};
pub use time::{CivilDate, Clock, FixedClock, SystemClock, Timestamp};
pub use units::{Area, Force, Length, Mass, Percent, Stress, Temperature, Unit};
