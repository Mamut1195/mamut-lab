//! `mamut-equipment` — instrumentos y calibraciones.
//!
//! Un resultado medido con un equipo fuera de calibración es un resultado en duda. Este
//! crate calcula el estado de calibración de forma pura: recibe la fecha de hoy, no la
//! lee del reloj, para que las pruebas sean deterministas.

use mamut_core::{CivilDate, EquipmentId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EquipmentState {
    Active,
    OutOfService,
    Retired,
}

impl EquipmentState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Operativo",
            Self::OutOfService => "Fuera de servicio",
            Self::Retired => "Retirado",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::OutOfService => "out_of_service",
            Self::Retired => "retired",
        }
    }
}

/// Estado de calibración respecto a una fecha de referencia.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalibrationStatus {
    /// Falta para el vencimiento más que el umbral de aviso.
    Ok { days_left: i64 },
    /// Vence dentro del umbral de aviso (incluido hoy).
    DueSoon { days_left: i64 },
    /// Vencida.
    Overdue { days_overdue: i64 },
    /// No hay fecha registrada: no se puede afirmar nada.
    Unknown,
}

impl CalibrationStatus {
    pub fn label(self) -> String {
        match self {
            Self::Ok { days_left } => format!("En vigor ({days_left} días)"),
            Self::DueSoon { days_left } => format!("Vence en {days_left} días"),
            Self::Overdue { days_overdue } => format!("Vencida hace {days_overdue} días"),
            Self::Unknown => "Sin fecha de calibración".to_owned(),
        }
    }

    /// Si el equipo puede usarse sin reservas.
    pub fn is_valid(self) -> bool {
        matches!(self, Self::Ok { .. } | Self::DueSoon { .. })
    }
}

/// Un equipo del laboratorio.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Equipment {
    pub id: EquipmentId,
    pub name: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub acquired_on: Option<CivilDate>,
    pub calibrated_on: Option<CivilDate>,
    pub next_calibration: Option<CivilDate>,
    pub responsible: Option<String>,
    pub state: EquipmentState,
}

impl Equipment {
    pub fn new(name: impl Into<String>) -> Option<Self> {
        let name = name.into().trim().to_owned();
        if name.is_empty() {
            return None;
        }
        Some(Self {
            id: EquipmentId::new(),
            name,
            brand: None,
            model: None,
            serial: None,
            acquired_on: None,
            calibrated_on: None,
            next_calibration: None,
            responsible: None,
            state: EquipmentState::Active,
        })
    }

    /// Estado de calibración a `today`.
    ///
    /// `warning_days` es una **política del laboratorio**, no un criterio normativo: cada
    /// organización decide con cuánta antelación quiere el aviso.
    pub fn calibration_status(&self, today: CivilDate, warning_days: i64) -> CalibrationStatus {
        let Some(next) = self.next_calibration else {
            return CalibrationStatus::Unknown;
        };
        let days_left = today.days_until(next);
        if days_left < 0 {
            CalibrationStatus::Overdue {
                days_overdue: -days_left,
            }
        } else if days_left <= warning_days {
            CalibrationStatus::DueSoon { days_left }
        } else {
            CalibrationStatus::Ok { days_left }
        }
    }

    /// Si el equipo no debe usarse para un ensayo hoy.
    ///
    /// Se bloquea cuando el equipo está retirado o fuera de servicio, cuando la
    /// calibración está vencida **y también cuando no hay fecha de calibración**: si no se
    /// puede demostrar que está calibrado, no se puede afirmar que lo esté. La política es
    /// deliberadamente conservadora; un laboratorio que quiera admitir equipos sin
    /// calibración registrada tendrá que decirlo de forma explícita.
    pub fn is_blocked(&self, today: CivilDate, warning_days: i64) -> bool {
        if matches!(
            self.state,
            EquipmentState::Retired | EquipmentState::OutOfService
        ) {
            return true;
        }
        matches!(
            self.calibration_status(today, warning_days),
            CalibrationStatus::Overdue { .. } | CalibrationStatus::Unknown
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> CivilDate {
        CivilDate::new(y, m, d).unwrap()
    }

    fn equipment_with_next(next: Option<CivilDate>) -> Equipment {
        let mut e = Equipment::new("Máquina universal").unwrap();
        e.next_calibration = next;
        e
    }

    #[test]
    fn un_equipo_sin_nombre_no_existe() {
        assert!(Equipment::new("   ").is_none());
        assert!(Equipment::new("Prensa").is_some());
    }

    #[test]
    fn sin_fecha_de_calibracion_no_se_afirma_nada() {
        let e = equipment_with_next(None);
        assert_eq!(
            e.calibration_status(date(2026, 11, 18), 30),
            CalibrationStatus::Unknown
        );
    }

    #[test]
    fn calibracion_lejana_esta_en_vigor() {
        let e = equipment_with_next(Some(date(2027, 6, 1)));
        assert_eq!(
            e.calibration_status(date(2026, 11, 18), 30),
            CalibrationStatus::Ok { days_left: 195 }
        );
        assert!(!e.is_blocked(date(2026, 11, 18), 30));
    }

    #[test]
    fn calibracion_dentro_del_umbral_avisa() {
        let e = equipment_with_next(Some(date(2026, 12, 1)));
        assert_eq!(
            e.calibration_status(date(2026, 11, 18), 30),
            CalibrationStatus::DueSoon { days_left: 13 }
        );
        assert!(e.calibration_status(date(2026, 11, 18), 30).is_valid());
    }

    #[test]
    fn calibracion_vencida_bloquea_el_uso() {
        let e = equipment_with_next(Some(date(2026, 11, 1)));
        assert_eq!(
            e.calibration_status(date(2026, 11, 18), 30),
            CalibrationStatus::Overdue { days_overdue: 17 }
        );
        assert!(e.is_blocked(date(2026, 11, 18), 30));
    }

    #[test]
    fn el_mismo_dia_del_vencimiento_avisa_no_vence() {
        let e = equipment_with_next(Some(date(2026, 11, 18)));
        assert_eq!(
            e.calibration_status(date(2026, 11, 18), 30),
            CalibrationStatus::DueSoon { days_left: 0 }
        );
        assert!(e.calibration_status(date(2026, 11, 18), 30).is_valid());
    }

    #[test]
    fn el_umbral_es_configurable() {
        let e = equipment_with_next(Some(date(2026, 12, 20)));
        // 32 días: fuera de un umbral de 30, dentro de uno de 60.
        assert!(matches!(
            e.calibration_status(date(2026, 11, 18), 30),
            CalibrationStatus::Ok { .. }
        ));
        assert!(matches!(
            e.calibration_status(date(2026, 11, 18), 60),
            CalibrationStatus::DueSoon { .. }
        ));
    }

    #[test]
    fn un_equipo_sin_fecha_de_calibracion_esta_bloqueado() {
        let e = equipment_with_next(None);
        assert!(
            e.is_blocked(date(2026, 11, 18), 30),
            "si no se puede demostrar que está calibrado, no se puede afirmar que lo esté"
        );
    }

    #[test]
    fn un_equipo_fuera_de_servicio_esta_bloqueado() {
        let mut e = equipment_with_next(Some(date(2030, 1, 1)));
        e.state = EquipmentState::OutOfService;
        assert!(
            e.is_blocked(date(2026, 11, 18), 30),
            "«fuera de servicio» debe impedir el uso, no solo «retirado»"
        );
    }

    #[test]
    fn un_equipo_retirado_esta_bloqueado_aunque_su_calibracion_este_en_vigor() {
        let mut e = equipment_with_next(Some(date(2030, 1, 1)));
        e.state = EquipmentState::Retired;
        assert!(e.is_blocked(date(2026, 11, 18), 30));
    }

    #[test]
    fn las_etiquetas_son_legibles_para_el_tecnico() {
        assert_eq!(
            CalibrationStatus::Overdue { days_overdue: 3 }.label(),
            "Vencida hace 3 días"
        );
        assert_eq!(EquipmentState::Active.label(), "Operativo");
        assert_eq!(EquipmentState::Active.as_str(), "active");
    }
}
