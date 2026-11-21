//! La muestra: lo que entra al laboratorio y a lo que todo se puede rastrear.

use serde::{Deserialize, Serialize};

use mamut_core::{Actor, CivilDate, Error as CoreError, SampleId, Timestamp};

use crate::{DomainError, Result};

/// Tipo de material. Determina qué ensayos tienen sentido.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Material {
    #[serde(rename = "concrete")]
    Concrete,
    #[serde(rename = "soil")]
    Soil,
    #[serde(rename = "aggregate")]
    Aggregate,
    #[serde(rename = "asphalt")]
    Asphalt,
}

impl Material {
    pub fn label(self) -> &'static str {
        match self {
            Self::Concrete => "Hormigón",
            Self::Soil => "Suelo",
            Self::Aggregate => "Agregado",
            Self::Asphalt => "Asfalto",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Concrete => "concrete",
            Self::Soil => "soil",
            Self::Aggregate => "aggregate",
            Self::Asphalt => "asphalt",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "concrete" => Self::Concrete,
            "soil" => Self::Soil,
            "aggregate" => Self::Aggregate,
            "asphalt" => Self::Asphalt,
            _ => return None,
        })
    }
}

/// Código visible de la muestra. Es lo que el cliente cita por teléfono.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SampleCode(String);

impl SampleCode {
    pub const MAX_LEN: usize = 40;

    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let code = raw.into().trim().to_ascii_uppercase();
        if code.is_empty() {
            return Err(CoreError::Empty { field: "código" }.into());
        }
        if code.len() > Self::MAX_LEN {
            return Err(CoreError::TooLong {
                field: "código",
                len: code.len(),
                max: Self::MAX_LEN,
            }
            .into());
        }
        if code.chars().any(char::is_whitespace) {
            return Err(CoreError::Message(
                "el código de muestra no puede contener espacios".into(),
            )
            .into());
        }
        Ok(Self(code))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SampleCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for SampleCode {
    type Error = DomainError;
    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SampleCode> for String {
    fn from(value: SampleCode) -> Self {
        value.0
    }
}

/// Estado de la muestra. `Voided` no borra: conserva la historia.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleState {
    Received,
    InTesting,
    Completed,
    Voided,
}

impl SampleState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Received => "Recibida",
            Self::InTesting => "En ensayo",
            Self::Completed => "Completada",
            Self::Voided => "Anulada",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::InTesting => "in_testing",
            Self::Completed => "completed",
            Self::Voided => "voided",
        }
    }

    /// Transiciones permitidas. Se escribe como tabla para que sea auditable de un
    /// vistazo y no dependa de condicionales dispersos.
    pub fn can_transition_to(self, next: Self) -> bool {
        use SampleState::*;
        matches!(
            (self, next),
            (Received, InTesting | Voided) | (InTesting, Completed | Voided) | (Completed, Voided)
        )
    }
}

/// Una muestra recepcionada.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub id: SampleId,
    pub code: SampleCode,
    pub material: Material,
    pub origin: String,
    pub client: String,
    pub project: String,
    pub received_on: CivilDate,
    pub responsible: Actor,
    pub state: SampleState,
    pub created_at: Timestamp,
    /// Se incrementa en cada cambio. Permite detectar ediciones concurrentes.
    pub revision: u32,
}

impl Sample {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        code: SampleCode,
        material: Material,
        origin: impl Into<String>,
        client: impl Into<String>,
        project: impl Into<String>,
        received_on: CivilDate,
        responsible: Actor,
        created_at: Timestamp,
    ) -> Result<Self> {
        let origin = required(origin.into(), "procedencia")?;
        let client = required(client.into(), "cliente")?;
        let project = required(project.into(), "proyecto")?;
        Ok(Self {
            id: SampleId::new(),
            code,
            material,
            origin,
            client,
            project,
            received_on,
            responsible,
            state: SampleState::Received,
            created_at,
            revision: 1,
        })
    }

    /// Cambia de estado si la transición está permitida.
    pub fn transition(&mut self, next: SampleState) -> Result<()> {
        if !self.state.can_transition_to(next) {
            return Err(DomainError::InvalidTransition {
                from: self.state.label(),
                to: next.label(),
            });
        }
        self.state = next;
        self.revision += 1;
        Ok(())
    }

    /// Si la muestra admite nuevos especímenes.
    pub fn accepts_specimens(&self) -> bool {
        matches!(self.state, SampleState::Received | SampleState::InTesting)
    }
}

fn required(value: String, field: &'static str) -> Result<String> {
    let trimmed = value.trim().to_owned();
    if trimmed.is_empty() {
        return Err(CoreError::Empty { field }.into());
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor() -> Actor {
        Actor::new("u-1", "Ana Torres").unwrap()
    }

    fn date() -> CivilDate {
        CivilDate::new(2026, 11, 18).unwrap()
    }

    fn sample() -> Sample {
        Sample::new(
            SampleCode::new("H-2026-0001").unwrap(),
            Material::Concrete,
            "Bloque 3, planta 5",
            "Constructora Andina",
            "Torre Norte",
            date(),
            actor(),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .unwrap()
    }

    #[test]
    fn el_codigo_se_normaliza_y_no_admite_espacios() {
        assert_eq!(
            SampleCode::new(" h-2026-0001 ").unwrap().as_str(),
            "H-2026-0001"
        );
        assert!(SampleCode::new("H 2026 0001").is_err());
        assert!(SampleCode::new("   ").is_err());
    }

    #[test]
    fn una_muestra_exige_procedencia_cliente_y_proyecto() {
        let code = SampleCode::new("H-1").unwrap();
        let r = Sample::new(
            code,
            Material::Concrete,
            "  ",
            "Cliente",
            "Proyecto",
            date(),
            actor(),
            Timestamp::from_millis(0),
        );
        assert!(r.is_err());
    }

    #[test]
    fn una_muestra_nueva_esta_recibida_y_en_revision_1() {
        let s = sample();
        assert_eq!(s.state, SampleState::Received);
        assert_eq!(s.revision, 1);
        assert!(s.accepts_specimens());
    }

    #[test]
    fn las_transiciones_permitidas_avanzan_y_suben_la_revision() {
        let mut s = sample();
        s.transition(SampleState::InTesting).unwrap();
        assert_eq!(s.state, SampleState::InTesting);
        assert_eq!(s.revision, 2);
        s.transition(SampleState::Completed).unwrap();
        assert_eq!(s.revision, 3);
        assert!(!s.accepts_specimens());
    }

    #[test]
    fn una_transicion_imposible_no_cambia_el_estado() {
        let mut s = sample();
        let antes = s.clone();
        assert!(matches!(
            s.transition(SampleState::Completed),
            Err(DomainError::InvalidTransition { .. })
        ));
        assert_eq!(s.state, antes.state);
        assert_eq!(
            s.revision, antes.revision,
            "un intento fallido no es un cambio"
        );
    }

    #[test]
    fn una_muestra_anulada_no_vuelve_atras() {
        use SampleState::*;
        assert!(!Voided.can_transition_to(Received));
        assert!(!Voided.can_transition_to(InTesting));
        assert!(!Voided.can_transition_to(Completed));
        assert!(Completed.can_transition_to(Voided));
    }

    #[test]
    fn el_estado_se_guarda_con_texto_estable() {
        for state in [
            SampleState::Received,
            SampleState::InTesting,
            SampleState::Completed,
            SampleState::Voided,
        ] {
            assert!(!state.as_str().is_empty());
        }
        assert_eq!(Material::parse("soil"), Some(Material::Soil));
        assert_eq!(Material::parse("madera"), None);
    }
}
