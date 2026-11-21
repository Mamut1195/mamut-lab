//! Compresión de cilindros de hormigón.
//!
//! **Qué se calcula aquí y qué no.** Se calculan magnitudes derivadas de la geometría y
//! de la carga: área de sección, tensión, densidad y esbeltez. No se decide si el
//! hormigón cumple: eso exige una especificación aplicable y una norma revisada, y vive
//! en `mamut-standards`.
//!
//! **Procedencia de las fórmulas.** Son definiciones de la magnitud, no criterios
//! normativos:
//! - Área de un círculo: `A = π·d²/4`.
//! - Tensión: `σ = F / A`.
//! - Densidad: `ρ = m / V`, con `V = π·d²·h/4`.
//!
//! El **procedimiento** del ensayo (velocidad de carga, refrentado, corrección por
//! esbeltez, número de especímenes, redondeo normativo) **no** está implementado: figura
//! como pendiente en el *norm pack* (ADR-0009, `docs/normas/packs/pack-inicial.json`).

use serde::Serialize;

use mamut_core::{Area, Force, Length, Mass, Stress};
use mamut_standards::NormRef;

use crate::rounding::RoundingPolicy;
use crate::{CalcError, Result};

/// Versión del motor de cálculo. **Cambiarla cambia la trazabilidad**: un resultado
/// guardado con otra versión se puede reproducir y explicar (ADR-0006 §3).
pub const ENGINE_VERSION: &str = "0.1.0";

/// Entradas del ensayo, con unidades tipadas.
#[derive(Clone, Debug, PartialEq)]
pub struct CylinderCompressionInput {
    pub diameter: Length,
    pub height: Length,
    pub mass: Mass,
    pub max_load: Force,
    pub age_days: u32,
    pub norm: NormRef,
    pub rounding: RoundingPolicy,
}

/// *Snapshot* canónico para la huella. Se construye **explícitamente**, con unidades
/// canónicas fijas, para que la huella no dependa de cómo se serialice una magnitud.
#[derive(Serialize)]
struct InputSnapshot {
    diameter_mm: f64,
    height_mm: f64,
    mass_kg: f64,
    max_load_n: f64,
    age_days: u32,
    norm_citation: String,
}

/// Resultado del cálculo, con todo lo necesario para auditarlo.
#[derive(Clone, Debug)]
pub struct CompressionOutcome {
    pub area: Area,
    pub stress: Stress,
    /// Densidad aparente del espécimen en kg/m³.
    pub density_kg_m3: f64,
    /// Relación altura/diámetro (esbeltez geométrica).
    pub slenderness: f64,
    /// Tensión redondeada **solo para presentación**.
    pub stress_display: f64,
    pub engine_version: &'static str,
    pub inputs_fingerprint: String,
    pub norm_citation: String,
}

impl CylinderCompressionInput {
    /// Huella SHA-256 de las entradas, en su forma canónica.
    pub fn fingerprint(&self) -> Result<String> {
        let snapshot = InputSnapshot {
            diameter_mm: self.diameter.millimetres_value(),
            height_mm: self.height.millimetres_value(),
            mass_kg: self.mass.canonical(),
            max_load_n: self.max_load.canonical(),
            age_days: self.age_days,
            norm_citation: self.norm.citation(),
        };
        mamut_core::hash::fingerprint(&snapshot).map_err(CalcError::Core)
    }
}

/// Calcula el resultado del ensayo.
pub fn calculate(input: &CylinderCompressionInput) -> Result<CompressionOutcome> {
    let area = Area::of_circle(input.diameter)?;
    let stress = Stress::from_force_over_area(input.max_load, area)?;

    // Volumen del cilindro en m³.
    let volume = {
        let d = input.diameter.canonical();
        let h = input.height.canonical();
        std::f64::consts::PI * d * d * h / 4.0
    };
    if !volume.is_finite() || volume <= 0.0 {
        return Err(CalcError::Incoherent(
            "el volumen del espécimen no es positivo".into(),
        ));
    }
    let density_kg_m3 = input.mass.canonical() / volume;
    let slenderness = input.height.canonical() / input.diameter.canonical();

    let inputs_fingerprint = input.fingerprint()?;

    Ok(CompressionOutcome {
        area,
        stress,
        density_kg_m3,
        slenderness,
        stress_display: input.rounding.apply(stress.megapascals_value()),
        engine_version: ENGINE_VERSION,
        inputs_fingerprint,
        norm_citation: input.norm.citation(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mamut_core::CivilDate;
    use mamut_standards::{NormBody, NormStatus};

    fn norm() -> NormRef {
        NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2026,
            "Compresión de cilindros",
            NormStatus::PendingReview,
        )
        .unwrap()
    }

    fn input() -> CylinderCompressionInput {
        CylinderCompressionInput {
            diameter: Length::millimetres(150.0).unwrap(),
            height: Length::millimetres(300.0).unwrap(),
            mass: Mass::grams(12_300.0).unwrap(),
            max_load: Force::kilonewtons(500.0).unwrap(),
            age_days: 28,
            norm: norm(),
            rounding: RoundingPolicy::PROVISIONAL,
        }
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1.0)
    }

    #[test]
    fn cilindro_150x300_con_500_kn() {
        let out = calculate(&input()).unwrap();
        // A = 17 671.4587 mm²
        assert!(close(
            out.area.square_millimetres_value(),
            17_671.458_676,
            1e-9
        ));
        // σ = 28.2942121 MPa
        assert!(close(out.stress.megapascals_value(), 28.294_212_1, 1e-6));
        // Esbeltez = 300/150 = 2
        assert!(close(out.slenderness, 2.0, 1e-12));
        // V = 0.005301438 m³ ; ρ = 12.3 / V = 2320.1 kg/m³
        assert!(close(out.density_kg_m3, 2_320.1, 1e-3));
    }

    #[test]
    fn el_valor_de_presentacion_esta_redondeado_y_el_calculo_no() {
        let out = calculate(&input()).unwrap();
        assert_eq!(out.stress_display, 28.29);
        assert!(
            (out.stress.megapascals_value() - 28.294_212_1).abs() < 1e-5,
            "el valor de cálculo conserva la precisión completa"
        );
        assert_ne!(
            out.stress.megapascals_value(),
            out.stress_display,
            "presentar no es calcular"
        );
    }

    #[test]
    fn la_huella_es_estable_y_sensible_a_las_entradas() {
        let a = input();
        let b = input();
        assert_eq!(a.fingerprint().unwrap(), b.fingerprint().unwrap());

        let mut c = input();
        c.max_load = Force::kilonewtons(500.1).unwrap();
        assert_ne!(a.fingerprint().unwrap(), c.fingerprint().unwrap());
    }

    #[test]
    fn la_huella_es_un_sha256_hexadecimal() {
        let h = input().fingerprint().unwrap();
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn un_diametro_nulo_no_produce_calculo() {
        let mut i = input();
        i.diameter = Length::millimetres(0.0).unwrap();
        assert!(calculate(&i).is_err());
    }

    #[test]
    fn el_resultado_declara_su_motor_y_su_norma() {
        let out = calculate(&input()).unwrap();
        assert_eq!(out.engine_version, ENGINE_VERSION);
        assert_eq!(out.norm_citation, "ASTM C39/C39M-26");
    }

    #[test]
    fn el_estado_de_revision_no_altera_la_huella_ni_el_numero() {
        // Revisar una norma no cambia la aritmética: la huella identifica las entradas
        // y la edición aplicada, no el estado administrativo de la revisión. Si el
        // estado cambiara la huella, un mismo cálculo dejaría de ser comparable
        // consigo mismo tras una revisión de oficina.
        let a = calculate(&input()).unwrap();

        let mut i = input();
        i.norm = NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2026,
            "Compresión de cilindros",
            NormStatus::Verified {
                reviewed_by: "Ing. Ana Torres".into(),
                reviewed_on: CivilDate::new(2026, 11, 20).unwrap(),
            },
        )
        .unwrap();
        let b = calculate(&i).unwrap();

        assert!(close(
            a.stress.megapascals_value(),
            b.stress.megapascals_value(),
            1e-12
        ));
        assert_eq!(a.inputs_fingerprint, b.inputs_fingerprint);
        assert_eq!(a.norm_citation, b.norm_citation);
    }

    #[test]
    fn cambiar_la_edicion_de_la_norma_si_cambia_la_huella() {
        // Distinto año = distinta edición = entradas distintas.
        let mut i = input();
        i.norm = NormRef::new(
            NormBody::Astm,
            "C39/C39M",
            2025,
            "Compresión de cilindros",
            NormStatus::PendingReview,
        )
        .unwrap();
        assert_ne!(
            calculate(&input()).unwrap().inputs_fingerprint,
            calculate(&i).unwrap().inputs_fingerprint
        );
    }
}
