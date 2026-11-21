//! El espécimen: la probeta concreta que se rompe en la máquina.
//!
//! Separar muestra y espécimen no es academicismo: una muestra de hormigón produce
//! varios cilindros, y cada uno se ensaya por separado. Sin esta separación no se puede
//! responder «¿qué cilindro dio 18 MPa?».

use mamut_core::{
    Area, CivilDate, Error as CoreError, Length, Mass, SampleId, SpecimenId, Timestamp,
};

use crate::Result;

/// Edad máxima admitida en días (10 años). Es una cota de cordura contra errores de
/// tecleo, no un límite normativo.
pub const MAX_AGE_DAYS: u32 = 3_650;

/// Un espécimen perteneciente a una muestra.
#[derive(Clone, Debug, PartialEq)]
pub struct Specimen {
    pub id: SpecimenId,
    pub sample_id: SampleId,
    /// Designación dentro de la muestra: `C1`, `C2`, `A`.
    pub designation: String,
    pub diameter: Length,
    pub height: Length,
    pub mass: Mass,
    pub age_days: u32,
    pub created_at: Timestamp,
}

impl Specimen {
    pub fn new(
        sample_id: SampleId,
        designation: impl Into<String>,
        diameter: Length,
        height: Length,
        mass: Mass,
        age_days: u32,
        created_at: Timestamp,
    ) -> Result<Self> {
        let designation = designation.into().trim().to_owned();
        if designation.is_empty() {
            return Err(CoreError::Empty {
                field: "designación del espécimen",
            }
            .into());
        }
        if !diameter.is_positive() {
            return Err(CoreError::NotPositive {
                field: "diámetro",
                value: diameter.millimetres_value(),
            }
            .into());
        }
        if !height.is_positive() {
            return Err(CoreError::NotPositive {
                field: "altura",
                value: height.millimetres_value(),
            }
            .into());
        }
        if !mass.is_positive() {
            return Err(CoreError::NotPositive {
                field: "masa",
                value: mass.grams_value(),
            }
            .into());
        }
        if age_days > MAX_AGE_DAYS {
            return Err(CoreError::Message(format!(
                "edad implausible: {age_days} días (máximo {MAX_AGE_DAYS})"
            ))
            .into());
        }
        Ok(Self {
            id: SpecimenId::new(),
            sample_id,
            designation,
            diameter,
            height,
            mass,
            age_days,
            created_at,
        })
    }

    /// Área de la sección transversal, a partir del diámetro.
    pub fn section_area(&self) -> Result<Area> {
        Ok(Area::of_circle(self.diameter)?)
    }
}

/// Fecha de ensayo sugerida a partir de la recepción y la edad. Utilidad de interfaz,
/// no una regla normativa.
pub fn suggested_test_date(received_on: CivilDate, age_days: u32) -> CivilDate {
    received_on.add_days(i64::from(age_days))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> Timestamp {
        Timestamp::from_millis(1_700_000_000_000)
    }

    fn cyl(diameter_mm: f64, height_mm: f64) -> Result<Specimen> {
        Specimen::new(
            SampleId::new(),
            "C1",
            Length::millimetres(diameter_mm)?,
            Length::millimetres(height_mm)?,
            Mass::grams(12_300.0)?,
            28,
            ts(),
        )
    }

    #[test]
    fn un_cilindro_de_150x300_es_valido() {
        let s = cyl(150.0, 300.0).unwrap();
        assert_eq!(s.designation, "C1");
        assert_eq!(s.age_days, 28);
    }

    #[test]
    fn una_dimension_cero_no_es_un_espécimen() {
        assert!(cyl(0.0, 300.0).is_err());
        assert!(cyl(150.0, 0.0).is_err());
    }

    #[test]
    fn una_masa_cero_no_es_un_espécimen() {
        let r = Specimen::new(
            SampleId::new(),
            "C1",
            Length::millimetres(150.0).unwrap(),
            Length::millimetres(300.0).unwrap(),
            Mass::grams(0.0).unwrap(),
            28,
            ts(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn una_designacion_vacia_no_es_un_espécimen() {
        let r = Specimen::new(
            SampleId::new(),
            "  ",
            Length::millimetres(150.0).unwrap(),
            Length::millimetres(300.0).unwrap(),
            Mass::grams(12_300.0).unwrap(),
            28,
            ts(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn una_edad_absurda_se_rechaza() {
        let r = Specimen::new(
            SampleId::new(),
            "C1",
            Length::millimetres(150.0).unwrap(),
            Length::millimetres(300.0).unwrap(),
            Mass::grams(12_300.0).unwrap(),
            MAX_AGE_DAYS + 1,
            ts(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn el_area_de_seccion_sale_del_diametro() {
        let s = cyl(150.0, 300.0).unwrap();
        let area = s.section_area().unwrap();
        assert!((area.square_millimetres_value() - 17_671.458_676).abs() < 1e-6);
    }

    #[test]
    fn la_fecha_de_ensayo_sugerida_suma_la_edad() {
        let d = CivilDate::new(2026, 11, 1).unwrap();
        assert_eq!(
            suggested_test_date(d, 28).to_iso(),
            "2026-11-29",
            "28 días después del 1 de noviembre"
        );
    }
}
