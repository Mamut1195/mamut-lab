//! El ensayo y su ciclo de vida.
//!
//! El estado no es decorativo: **no se puede informar un ensayo sin resultado
//! calculado**, y un ensayo informado ya no se modifica. Eso se garantiza aquí, antes
//! de que exista una pantalla o una base de datos.

use serde::{Deserialize, Serialize};

use mamut_core::{Actor, CivilDate, Error as CoreError, SpecimenId, TestId, Timestamp, Unit};
use mamut_standards::TestDescriptor;

use crate::{DomainError, Result};

/// Referencia al resultado calculado que respalda un ensayo.
///
/// No es el resultado «en crudo»: es lo mínimo que permite auditar de dónde salió el
/// número. El cálculo completo vive en `mamut-calculations`; aquí solo se guarda la
/// referencia que lo hace verificable.
///
/// `#[non_exhaustive]`: fuera de este crate no se puede construir con un literal, así que
/// un resultado no puede entrar sin pasar por [`ResultRef::new`] (valor finito, unidad
/// admitida, huella de 64 hexadecimales).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResultRef {
    pub value: f64,
    /// Unidad del valor, explícita y sin ambigüedad.
    pub unit: String,
    /// Versión del motor de cálculo que lo produjo.
    pub engine_version: String,
    /// SHA-256 de las entradas exactas (64 caracteres hexadecimales).
    pub inputs_fingerprint: String,
    pub calculated_at: Timestamp,
}

impl ResultRef {
    pub fn new(
        value: f64,
        unit: impl Into<String>,
        engine_version: impl Into<String>,
        inputs_fingerprint: impl Into<String>,
        calculated_at: Timestamp,
    ) -> Result<Self> {
        let raw_unit = unit.into();
        let raw_unit = raw_unit.trim();
        let this = Self {
            value,
            // Símbolo canónico: «mpa» y «MPa» son la misma unidad y la base solo admite
            // la segunda.
            unit: Unit::parse(raw_unit)
                .map(|unit| unit.symbol().to_owned())
                .unwrap_or_else(|| raw_unit.to_owned()),
            engine_version: engine_version.into().trim().to_owned(),
            inputs_fingerprint: inputs_fingerprint.into().trim().to_ascii_lowercase(),
            calculated_at,
        };
        this.validate()?;
        Ok(this)
    }

    /// Revalida los invariantes del resultado.
    ///
    /// Se usa al **leer de la base**: un resultado con unidad inventada o valor infinito
    /// no debe llegar a un informe, y además una vez insertado sería irreparable porque
    /// la tabla no admite modificaciones.
    pub fn validate(&self) -> Result<()> {
        if !self.value.is_finite() {
            return Err(CoreError::NonFinite { field: "resultado" }.into());
        }
        if self.unit.is_empty() {
            return Err(CoreError::Empty {
                field: "unidad del resultado",
            }
            .into());
        }
        if Unit::parse(&self.unit).is_none() {
            return Err(CoreError::Message(format!(
                "unidad desconocida en el resultado: «{}»",
                self.unit
            ))
            .into());
        }
        if self.engine_version.is_empty() {
            return Err(CoreError::Empty {
                field: "versión del motor de cálculo",
            }
            .into());
        }
        if self.inputs_fingerprint.len() != 64
            || !self
                .inputs_fingerprint
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        {
            return Err(CoreError::Message(format!(
                "huella de entradas inválida: «{}» (se esperan 64 caracteres hexadecimales)",
                self.inputs_fingerprint
            ))
            .into());
        }
        Ok(())
    }
}

/// Estado del ensayo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestState {
    Draft,
    Registered,
    Calculated,
    Reported,
    Voided,
}

impl TestState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "Borrador",
            Self::Registered => "Registrado",
            Self::Calculated => "Calculado",
            Self::Reported => "Informado",
            Self::Voided => "Anulado",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Registered => "registered",
            Self::Calculated => "calculated",
            Self::Reported => "reported",
            Self::Voided => "voided",
        }
    }

    /// Tabla de transiciones. `Calculated` y `Reported` no se alcanzan con
    /// [`TestRecord::transition`]: exigen un resultado, y eso lo imponen
    /// [`TestRecord::set_result`] y [`TestRecord::report`].
    pub fn can_transition_to(self, next: Self) -> bool {
        use TestState::*;
        matches!(
            (self, next),
            (Draft, Registered | Voided)
                | (Registered, Voided)
                | (Calculated, Voided)
                | (Reported, Voided)
        )
    }
}

/// Un ensayo sobre un espécimen.
#[derive(Clone, Debug, PartialEq)]
pub struct TestRecord {
    pub id: TestId,
    pub specimen_id: SpecimenId,
    pub descriptor: TestDescriptor,
    pub state: TestState,
    pub performed_on: CivilDate,
    /// Equipo utilizado. Obligatorio a partir de la fase 5; hoy se admite vacío.
    pub equipment: Option<String>,
    pub operator: Actor,
    pub created_at: Timestamp,
    pub revision: u32,
    pub result: Option<ResultRef>,
}

impl TestRecord {
    pub fn new(
        descriptor: TestDescriptor,
        specimen_id: SpecimenId,
        performed_on: CivilDate,
        operator: Actor,
        created_at: Timestamp,
    ) -> Self {
        Self {
            id: TestId::new(),
            specimen_id,
            descriptor,
            state: TestState::Draft,
            performed_on,
            equipment: None,
            operator,
            created_at,
            revision: 1,
            result: None,
        }
    }

    /// Cierra la captura: los datos originales quedan fijados.
    pub fn register(&mut self, equipment: Option<String>) -> Result<()> {
        if !self.state.can_transition_to(TestState::Registered) {
            return Err(DomainError::InvalidTransition {
                from: self.state.label(),
                to: TestState::Registered.label(),
            });
        }
        self.equipment = equipment
            .map(|e| e.trim().to_owned())
            .filter(|e| !e.is_empty());
        self.state = TestState::Registered;
        self.revision += 1;
        Ok(())
    }

    /// Adjunta el resultado calculado y pasa a `Calculated`.
    pub fn set_result(&mut self, result: ResultRef) -> Result<()> {
        if self.state != TestState::Registered {
            return Err(DomainError::InvalidTransition {
                from: self.state.label(),
                to: TestState::Calculated.label(),
            });
        }
        self.result = Some(result);
        self.state = TestState::Calculated;
        self.revision += 1;
        Ok(())
    }

    /// Marca el ensayo como informado. Exige resultado.
    pub fn report(&mut self) -> Result<()> {
        if self.result.is_none() {
            return Err(DomainError::MissingResult);
        }
        if self.state != TestState::Calculated {
            return Err(DomainError::InvalidTransition {
                from: self.state.label(),
                to: TestState::Reported.label(),
            });
        }
        self.state = TestState::Reported;
        self.revision += 1;
        Ok(())
    }

    /// Anula el ensayo sin borrarlo. El resultado, si existe, se conserva.
    pub fn void(&mut self) -> Result<()> {
        if !self.state.can_transition_to(TestState::Voided) {
            return Err(DomainError::InvalidTransition {
                from: self.state.label(),
                to: TestState::Voided.label(),
            });
        }
        self.state = TestState::Voided;
        self.revision += 1;
        Ok(())
    }

    /// Si el ensayo admite correcciones de datos.
    pub fn is_editable(&self) -> bool {
        matches!(self.state, TestState::Draft | TestState::Registered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mamut_standards::{MethodKind, NormBody, NormRef, NormStatus, TestCode};

    fn ts() -> Timestamp {
        Timestamp::from_millis(1_700_000_000_000)
    }

    fn actor() -> Actor {
        Actor::new("u-1", "Ana Torres").unwrap()
    }

    fn descriptor() -> TestDescriptor {
        let norm = NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2026,
            "Compresión de cilindros",
            NormStatus::PendingReview,
        )
        .unwrap();
        TestDescriptor::new(
            TestCode::new("CONC-COMP-CYL").unwrap(),
            "Compresión de cilindros",
            MethodKind::Normative,
            Some(norm),
            false,
        )
        .unwrap()
    }

    fn record() -> TestRecord {
        TestRecord::new(
            descriptor(),
            SpecimenId::new(),
            CivilDate::new(2026, 11, 18).unwrap(),
            actor(),
            ts(),
        )
    }

    fn result() -> ResultRef {
        ResultRef::new(28.29, "MPa", "calc-0.1.0", "a".repeat(64), ts()).unwrap()
    }

    #[test]
    fn un_ensayo_nace_en_borrador_y_sin_resultado() {
        let t = record();
        assert_eq!(t.state, TestState::Draft);
        assert!(t.result.is_none());
        assert!(t.is_editable());
    }

    #[test]
    fn no_se_puede_informar_sin_resultado() {
        let mut t = record();
        t.register(None).unwrap();
        assert_eq!(t.report(), Err(DomainError::MissingResult));
        assert_eq!(t.state, TestState::Registered);
    }

    #[test]
    fn el_ciclo_completo_funciona() {
        let mut t = record();
        t.register(Some("Máquina universal 1".into())).unwrap();
        assert_eq!(t.equipment.as_deref(), Some("Máquina universal 1"));
        t.set_result(result()).unwrap();
        assert_eq!(t.state, TestState::Calculated);
        assert!(!t.is_editable());
        t.report().unwrap();
        assert_eq!(t.state, TestState::Reported);
        assert_eq!(t.revision, 4, "tres transiciones desde la revisión inicial");
    }

    #[test]
    fn no_se_calcula_dos_veces_sin_volver_atras() {
        let mut t = record();
        t.register(None).unwrap();
        t.set_result(result()).unwrap();
        assert!(
            t.set_result(result()).is_err(),
            "un resultado no se sobrescribe"
        );
    }

    #[test]
    fn un_ensayo_informado_no_se_informa_otra_vez() {
        let mut t = record();
        t.register(None).unwrap();
        t.set_result(result()).unwrap();
        t.report().unwrap();
        assert!(t.report().is_err());
    }

    #[test]
    fn anular_conserva_el_resultado() {
        let mut t = record();
        t.register(None).unwrap();
        t.set_result(result()).unwrap();
        t.void().unwrap();
        assert_eq!(t.state, TestState::Voided);
        assert!(t.result.is_some(), "anular no borra la evidencia");
    }

    #[test]
    fn un_ensayo_anulado_no_vuelve_a_la_vida() {
        let mut t = record();
        t.void().unwrap();
        assert!(t.register(None).is_err());
    }

    #[test]
    fn un_resultado_exige_huella_de_64_hexadecimales() {
        assert!(
            ResultRef::new(1.0, "MPa", "v1", "abc", ts()).is_err(),
            "una huella corta no identifica nada"
        );
        assert!(ResultRef::new(1.0, "MPa", "v1", "z".repeat(64), ts()).is_err());
        assert!(ResultRef::new(1.0, "MPa", "v1", "A".repeat(64), ts()).is_ok());
    }

    #[test]
    fn un_resultado_exige_unidad_y_version() {
        assert!(ResultRef::new(1.0, "  ", "v1", "a".repeat(64), ts()).is_err());
        assert!(ResultRef::new(1.0, "MPa", "  ", "a".repeat(64), ts()).is_err());
    }

    #[test]
    fn un_resultado_no_finito_no_es_resultado() {
        assert!(ResultRef::new(f64::NAN, "MPa", "v1", "a".repeat(64), ts()).is_err());
        assert!(ResultRef::new(f64::INFINITY, "MPa", "v1", "a".repeat(64), ts()).is_err());
    }

    #[test]
    fn un_resultado_con_unidad_inventada_no_es_resultado() {
        assert!(ResultRef::new(28.0, "banana", "v1", "a".repeat(64), ts()).is_err());
        assert!(ResultRef::new(28.0, "MPa", "v1", "a".repeat(64), ts()).is_ok());
    }

    #[test]
    fn la_unidad_del_resultado_se_guarda_canonica() {
        let result = ResultRef::new(28.0, "mpa", "v1", "a".repeat(64), ts()).unwrap();
        assert_eq!(result.unit, "MPa");
    }

    #[test]
    fn validate_detecta_un_resultado_manipulado() {
        // Se construye uno válido y se corrompe un campo: la revalidación debe verlo, que
        // es lo que protege a una fila leída de la base.
        let mut result = ResultRef::new(28.0, "MPa", "v1", "a".repeat(64), ts()).unwrap();
        assert!(result.validate().is_ok());
        result.unit = "banana".into();
        assert!(result.validate().is_err());
        result.unit = "MPa".into();
        result.value = f64::INFINITY;
        assert!(result.validate().is_err());
    }
}
