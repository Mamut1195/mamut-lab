//! Auditoría: quién hizo qué, cuándo y sobre qué.
//!
//! El registro es **append-only**: la base de datos lo impone con disparadores
//! (ver `mamut-storage`), no una convención de programación. Un registro que se puede
//! editar no sirve como prueba.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::ids::AuditId;
use crate::time::Timestamp;

/// Quien ejecuta una acción. No se usa un `String` suelto para que no haya duda de
/// si el campo es un identificador o un nombre.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub id: String,
    pub name: String,
}

impl Actor {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Result<Self> {
        let id = id.into().trim().to_owned();
        let name = name.into().trim().to_owned();
        if id.is_empty() {
            return Err(Error::Empty { field: "actor.id" });
        }
        if name.is_empty() {
            return Err(Error::Empty {
                field: "actor.name",
            });
        }
        Ok(Self { id, name })
    }
}

/// Qué ocurrió. Los valores son estables y se guardan como texto en la base.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    Created,
    Updated,
    Corrected,
    Calculated,
    Reported,
    Voided,
    Imported,
    Migrated,
}

impl AuditAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Corrected => "corrected",
            Self::Calculated => "calculated",
            Self::Reported => "reported",
            Self::Voided => "voided",
            Self::Imported => "imported",
            Self::Migrated => "migrated",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "created" => Self::Created,
            "updated" => Self::Updated,
            "corrected" => Self::Corrected,
            "calculated" => Self::Calculated,
            "reported" => Self::Reported,
            "voided" => Self::Voided,
            "imported" => Self::Imported,
            "migrated" => Self::Migrated,
            _ => return None,
        })
    }
}

/// Una entrada del registro de auditoría.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: AuditId,
    pub at: Timestamp,
    pub actor: Actor,
    /// Tabla o agregado afectado (`sample`, `test`, `result`, …).
    pub entity: String,
    /// Identificador del registro afectado, como texto.
    pub entity_id: String,
    pub action: AuditAction,
    /// Contexto adicional (campos cambiados, motivo de corrección, …).
    pub detail: serde_json::Value,
}

impl AuditEntry {
    pub fn new(
        at: Timestamp,
        actor: Actor,
        entity: impl Into<String>,
        entity_id: impl Into<String>,
        action: AuditAction,
    ) -> Result<Self> {
        let entity = entity.into().trim().to_owned();
        let entity_id = entity_id.into().trim().to_owned();
        if entity.is_empty() {
            return Err(Error::Empty {
                field: "audit.entity",
            });
        }
        if entity_id.is_empty() {
            return Err(Error::Empty {
                field: "audit.entity_id",
            });
        }
        Ok(Self {
            id: AuditId::new(),
            at,
            actor,
            entity,
            entity_id,
            action,
            detail: serde_json::Value::Null,
        })
    }

    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail = detail;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor() -> Actor {
        Actor::new("u-1", "Ana Torres").unwrap()
    }

    #[test]
    fn un_actor_sin_nombre_no_existe() {
        assert!(Actor::new("u-1", "   ").is_err());
        assert!(Actor::new("", "Ana").is_err());
    }

    #[test]
    fn nombre_e_id_se_recortan() {
        let a = Actor::new("  u-1  ", "  Ana  ").unwrap();
        assert_eq!(a.id, "u-1");
        assert_eq!(a.name, "Ana");
    }

    #[test]
    fn las_acciones_van_y_vuelven_por_texto() {
        for action in [
            AuditAction::Created,
            AuditAction::Updated,
            AuditAction::Corrected,
            AuditAction::Calculated,
            AuditAction::Reported,
            AuditAction::Voided,
            AuditAction::Imported,
            AuditAction::Migrated,
        ] {
            assert_eq!(AuditAction::parse(action.as_str()), Some(action));
        }
        assert_eq!(AuditAction::parse("inventado"), None);
    }

    #[test]
    fn una_entrada_sin_entidad_no_se_crea() {
        let ts = Timestamp::from_millis(0);
        assert!(AuditEntry::new(ts, actor(), "", "x", AuditAction::Created).is_err());
        assert!(AuditEntry::new(ts, actor(), "sample", "  ", AuditAction::Created).is_err());
    }

    #[test]
    fn una_entrada_valida_lleva_su_detalle() {
        let ts = Timestamp::from_millis(1_700_000_000_000);
        let entry = AuditEntry::new(ts, actor(), "sample", "s-1", AuditAction::Corrected)
            .unwrap()
            .with_detail(serde_json::json!({"motivo": "lectura mal transcrita"}));
        assert_eq!(entry.action, AuditAction::Corrected);
        assert_eq!(entry.detail["motivo"], "lectura mal transcrita");
        assert_eq!(entry.entity, "sample");
    }
}
