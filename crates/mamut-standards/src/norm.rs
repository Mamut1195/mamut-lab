//! Referencias normativas.

use serde::{Deserialize, Serialize};

use mamut_core::CivilDate;

use crate::{MAX_PLAUSIBLE_YEAR, MIN_PLAUSIBLE_YEAR, Result, StandardsError};

/// Organismo que publica la norma.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormBody {
    #[serde(rename = "ASTM")]
    Astm,
    #[serde(rename = "AASHTO")]
    Aashto,
    #[serde(rename = "NTE_INEN")]
    NteInen,
    #[serde(rename = "ISO")]
    Iso,
    #[serde(rename = "INTERNAL")]
    Internal,
}

impl NormBody {
    pub fn label(self) -> &'static str {
        match self {
            Self::Astm => "ASTM",
            Self::Aashto => "AASHTO",
            Self::NteInen => "NTE INEN",
            Self::Iso => "ISO",
            Self::Internal => "INTERNO",
        }
    }
}

/// Estado de revisión de una norma.
///
/// `Verified` no es un booleano: obliga a nombrar al revisor y la fecha. No existe la
/// variante «verificada por nadie».
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormStatus {
    /// Se conoce el nombre pero no la edición vigente.
    #[serde(rename = "PendingMetadata")]
    PendingMetadata,
    /// Metadatos correctos; el contenido no está validado por un ingeniero.
    #[serde(rename = "PendingReview")]
    PendingReview,
    /// Revisada y firmada.
    #[serde(rename = "Verified")]
    Verified {
        reviewed_by: String,
        reviewed_on: CivilDate,
    },
}

impl NormStatus {
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified { .. })
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::PendingMetadata => "Pendiente de metadatos",
            Self::PendingReview => "Pendiente de revisión",
            Self::Verified { .. } => "Revisada",
        }
    }
}

/// Una referencia normativa concreta y versionada.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormRef {
    pub body: NormBody,
    /// Designación sin año: `C39/C39M`, `T 22M/T 22`, `1573`.
    pub designation: String,
    pub year: u16,
    pub title: String,
    pub status: NormStatus,
}

/// Si el año (o, en un método interno, la versión) es plausible.
///
/// Un método interno usa un número de versión (`v1`), no una edición: por eso el rango
/// es distinto y la cita resultante tampoco se disfraza de norma.
pub fn is_plausible_year(body: NormBody, year: u16) -> bool {
    match body {
        NormBody::Internal => (1..=999).contains(&year),
        _ => (MIN_PLAUSIBLE_YEAR..=MAX_PLAUSIBLE_YEAR).contains(&year),
    }
}

impl NormRef {
    pub fn new(
        body: NormBody,
        designation: impl Into<String>,
        year: u16,
        title: impl Into<String>,
        status: NormStatus,
    ) -> Result<Self> {
        let designation = designation.into().trim().to_owned();
        let title = title.into().trim().to_owned();
        if designation.is_empty() {
            return Err(StandardsError::Empty {
                field: "norm.designation",
            });
        }
        if title.is_empty() {
            return Err(StandardsError::Empty {
                field: "norm.title",
            });
        }
        if !is_plausible_year(body, year) {
            return Err(StandardsError::ImplausibleYear { year });
        }
        Ok(Self {
            body,
            designation,
            year,
            title,
            status,
        })
    }

    pub fn is_verified(&self) -> bool {
        self.status.is_verified()
    }

    /// Cita completa en el estilo de cada organismo.
    ///
    /// - ASTM: `ASTM C39/C39M-26`
    /// - AASHTO: `AASHTO T 22M/T 22-25`
    /// - NTE INEN: `NTE INEN 1573:2010`
    /// - ISO: `ISO 1234:2020`
    /// - interno: `INTERNO PROCTOR v1`
    pub fn citation(&self) -> String {
        let last_two = format!("{:02}", self.year % 100);
        match self.body {
            NormBody::Astm => format!("ASTM {}-{}", self.designation, last_two),
            NormBody::Aashto => format!("AASHTO {}-{}", self.designation, last_two),
            NormBody::NteInen => format!("NTE INEN {}:{}", self.designation, self.year),
            NormBody::Iso => format!("ISO {}:{}", self.designation, self.year),
            NormBody::Internal => format!("INTERNO {} v{}", self.designation, self.year),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending() -> NormStatus {
        NormStatus::PendingReview
    }

    #[test]
    fn cita_astm_con_dos_digitos() {
        let n = NormRef::new(NormBody::Astm, "C39/C39M", 2026, "Compresión", pending()).unwrap();
        assert_eq!(n.citation(), "ASTM C39/C39M-26");
    }

    #[test]
    fn cita_aashto() {
        let n = NormRef::new(
            NormBody::Aashto,
            "T 22M/T 22",
            2025,
            "Compresión",
            pending(),
        )
        .unwrap();
        assert_eq!(n.citation(), "AASHTO T 22M/T 22-25");
    }

    #[test]
    fn cita_nte_inen_con_anio_completo() {
        let n = NormRef::new(NormBody::NteInen, "1573", 2010, "Compresión", pending()).unwrap();
        assert_eq!(n.citation(), "NTE INEN 1573:2010");
    }

    #[test]
    fn cita_de_metodo_interno_no_se_disfraza_de_norma() {
        let n = NormRef::new(
            NormBody::Internal,
            "PROCTOR",
            1,
            "Método interno",
            pending(),
        )
        .unwrap();
        assert_eq!(n.citation(), "INTERNO PROCTOR v1");
    }

    #[test]
    fn una_designacion_vacia_no_es_una_norma() {
        assert!(NormRef::new(NormBody::Astm, "  ", 2026, "t", pending()).is_err());
        assert!(NormRef::new(NormBody::Astm, "C39", 2026, "", pending()).is_err());
    }

    #[test]
    fn un_anio_implausible_se_rechaza() {
        assert!(NormRef::new(NormBody::Astm, "C39", 1800, "t", pending()).is_err());
        assert!(NormRef::new(NormBody::Astm, "C39", 3000, "t", pending()).is_err());
        // Un método interno usa versión, no edición: v1 es válido.
        assert!(NormRef::new(NormBody::Internal, "PROCTOR", 1, "t", pending()).is_ok());
        assert!(NormRef::new(NormBody::Internal, "PROCTOR", 0, "t", pending()).is_err());
    }

    #[test]
    fn solo_verified_cuenta_como_revisada() {
        assert!(!NormStatus::PendingMetadata.is_verified());
        assert!(!NormStatus::PendingReview.is_verified());
        let verified = NormStatus::Verified {
            reviewed_by: "Ing. Ana Torres".into(),
            reviewed_on: CivilDate::new(2026, 11, 20).unwrap(),
        };
        assert!(verified.is_verified());
    }
}
