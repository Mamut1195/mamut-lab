//! Carga y validación del *norm pack*.
//!
//! El pack es el mecanismo que permite incorporar normas al final sin recompilar
//! (ADR-0009). Su valor depende de que **no se pueda cargar un pack incoherente**: un
//! pack que declare una norma «revisada» sin revisor, o un ensayo que dictamine
//! conformidad sin límites, es peor que no tener pack.
//!
//! Las reglas están en `docs/normas/README.md` y aquí se implementan una a una.

use std::collections::HashSet;

use mamut_core::CivilDate;
use serde::Deserialize;

use crate::{
    Conformity, MAX_PACK_VERSION, MethodKind, NormBody, NormRef, NormStatus, Result, SpecLimits,
    StandardsError, TestCode, TestDescriptor,
};

// ---------------------------------------------------------------- modelo ---

/// Estado tal como aparece en el JSON (sin fecha ni revisor: esos son campos aparte).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PackStatus {
    PendingMetadata,
    PendingReview,
    Verified,
}

/// Confianza en los metadatos: fuente oficial del organismo o fuente secundaria.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Official,
    #[default]
    Secondary,
}

/// Una norma dentro del pack.
#[derive(Clone, Debug, Deserialize)]
pub struct PackNorm {
    pub id: String,
    pub body: NormBody,
    pub designation: String,
    pub year: u16,
    pub title: String,
    #[serde(default)]
    pub scope_summary: Option<String>,
    pub status: PackStatus,
    pub metadata_source: String,
    pub metadata_checked_on: CivilDate,
    #[serde(default)]
    pub metadata_confidence: Confidence,
    #[serde(default)]
    pub reviewed_by: Option<String>,
    #[serde(default)]
    pub reviewed_on: Option<CivilDate>,
    #[serde(default)]
    pub clauses_implemented: Vec<String>,
    #[serde(default)]
    pub clauses_pending: Vec<String>,
    #[serde(default)]
    pub spec_limits: Option<SpecLimits>,
    #[serde(default)]
    pub notes: Option<String>,
}

impl PackNorm {
    /// Convierte la entrada del pack en una referencia normativa del dominio.
    pub fn to_norm_ref(&self) -> Result<NormRef> {
        let status = match self.status {
            PackStatus::PendingMetadata => NormStatus::PendingMetadata,
            PackStatus::PendingReview => NormStatus::PendingReview,
            PackStatus::Verified => {
                let reviewed_by = self
                    .reviewed_by
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or(StandardsError::VerifiedWithoutReview)?
                    .to_owned();
                let reviewed_on = self
                    .reviewed_on
                    .ok_or(StandardsError::VerifiedWithoutReview)?;
                NormStatus::Verified {
                    reviewed_by,
                    reviewed_on,
                }
            }
        };
        NormRef::new(
            self.body,
            self.designation.clone(),
            self.year,
            self.title.clone(),
            status,
        )
    }
}

/// Un ensayo declarado en el pack.
#[derive(Clone, Debug, Deserialize)]
pub struct PackTest {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub norm_id: Option<String>,
    pub method_kind: MethodKind,
    #[serde(default)]
    pub quantity: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub computation: Option<String>,
    #[serde(default)]
    pub conformity_enabled: bool,
    #[serde(default)]
    pub blocked_reason: Option<String>,
}

/// El pack completo.
#[derive(Clone, Debug, Deserialize)]
pub struct NormPack {
    pub pack_version: u32,
    pub pack_id: String,
    #[serde(default)]
    pub generated_on: Option<CivilDate>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub norms: Vec<PackNorm>,
    #[serde(default)]
    pub tests: Vec<PackTest>,
}

// ----------------------------------------------------------- diagnóstico ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackSeverity {
    Error,
    Warning,
}

/// Un problema detectado en el pack. `code` es estable para poder probarlo y mostrarlo.
#[derive(Clone, Debug, PartialEq)]
pub struct PackIssue {
    pub severity: PackSeverity,
    pub code: &'static str,
    pub message: String,
}

impl PackIssue {
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: PackSeverity::Error,
            code,
            message: message.into(),
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: PackSeverity::Warning,
            code,
            message: message.into(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == PackSeverity::Error
    }
}

impl std::fmt::Display for PackIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            PackSeverity::Error => "error",
            PackSeverity::Warning => "aviso",
        };
        write!(f, "[{}] {}: {}", self.code, sev, self.message)
    }
}

// ------------------------------------------------------------ validación ---

impl NormPack {
    pub fn norm(&self, id: &str) -> Option<&PackNorm> {
        self.norms.iter().find(|n| n.id == id)
    }

    /// Aplica todas las reglas de integridad del pack.
    pub fn validate(&self) -> Vec<PackIssue> {
        let mut issues = Vec::new();

        // E001 · versión de formato
        if self.pack_version > MAX_PACK_VERSION {
            issues.push(PackIssue::error(
                "E001",
                format!(
                    "el pack declara la versión {}; esta aplicación admite hasta la {}",
                    self.pack_version, MAX_PACK_VERSION
                ),
            ));
            // Sin entender el formato no tiene sentido seguir validando el contenido.
            return issues;
        }

        if self.pack_id.trim().is_empty() {
            issues.push(PackIssue::error("E000", "el pack no tiene identificador"));
        }

        // --- normas ---
        let mut seen: HashSet<(String, u16)> = HashSet::new();
        let mut seen_ids: HashSet<&str> = HashSet::new();

        for norm in &self.norms {
            if !seen_ids.insert(norm.id.as_str()) {
                issues.push(PackIssue::error(
                    "E012",
                    format!("identificador de norma repetido: «{}»", norm.id),
                ));
            }

            // E008 · campos obligatorios
            if norm.id.trim().is_empty() {
                issues.push(PackIssue::error("E008", "norma sin id"));
            }
            if norm.designation.trim().is_empty() {
                issues.push(PackIssue::error(
                    "E008",
                    format!("norma «{}» sin designación", norm.id),
                ));
            }
            if norm.title.trim().is_empty() {
                issues.push(PackIssue::error(
                    "E008",
                    format!("norma «{}» sin título", norm.id),
                ));
            }
            if norm.metadata_source.trim().is_empty() {
                issues.push(PackIssue::error(
                    "E011",
                    format!("norma «{}» sin fuente de metadatos", norm.id),
                ));
            }

            // E002 · duplicados por designación + año
            let key = (norm.designation.trim().to_ascii_uppercase(), norm.year);
            if !seen.insert(key.clone()) {
                issues.push(PackIssue::error(
                    "E002",
                    format!(
                        "norma duplicada: {} edición {}",
                        norm.designation, norm.year
                    ),
                ));
            }

            // E010 · año (o versión, si es interno) plausible
            if !crate::norm::is_plausible_year(norm.body, norm.year) {
                issues.push(PackIssue::error(
                    "E010",
                    format!("norma «{}» con año implausible: {}", norm.id, norm.year),
                ));
            }

            // E003 / E004 · verificación firmada
            if norm.status == PackStatus::Verified {
                if norm
                    .reviewed_by
                    .as_deref()
                    .map(str::trim)
                    .is_none_or(str::is_empty)
                {
                    issues.push(PackIssue::error(
                        "E003",
                        format!(
                            "norma «{}» marcada como revisada sin indicar revisor",
                            norm.id
                        ),
                    ));
                }
                if norm.reviewed_on.is_none() {
                    issues.push(PackIssue::error(
                        "E004",
                        format!(
                            "norma «{}» marcada como revisada sin fecha de revisión",
                            norm.id
                        ),
                    ));
                }
            }

            // E009 · límites coherentes
            if let Some(limits) = &norm.spec_limits {
                if limits.is_empty() {
                    issues.push(PackIssue::error(
                        "E009",
                        format!(
                            "norma «{}» declara límites vacíos; use null si no hay criterio",
                            norm.id
                        ),
                    ));
                }
                if let (Some(lo), Some(hi)) = (limits.min, limits.max)
                    && lo > hi
                {
                    issues.push(PackIssue::error(
                        "E009",
                        format!("norma «{}» con mínimo {lo} mayor que máximo {hi}", norm.id),
                    ));
                }
                if !limits.min.into_iter().chain(limits.max).all(f64::is_finite) {
                    issues.push(PackIssue::error(
                        "E009",
                        format!("norma «{}» con límites no finitos", norm.id),
                    ));
                }
            }
        }

        // --- ensayos ---
        let mut test_codes: HashSet<String> = HashSet::new();
        for test in &self.tests {
            let resolved = test.norm_id.as_deref().and_then(|id| self.norm(id));

            // E013 / E014 · el código del ensayo debe ser válido y único.
            // Sin esto, un código con espacios cargaba el pack sin diagnóstico y el
            // ensayo desaparecía en silencio de `descriptors()`.
            let normalized_code = test.code.trim().to_ascii_uppercase();
            if normalized_code.is_empty() {
                issues.push(PackIssue::error("E013", "ensayo sin código"));
            } else if TestCode::new(test.code.clone()).is_err() {
                issues.push(PackIssue::error(
                    "E013",
                    format!(
                        "código de ensayo inválido: «{}» (se admite A-Z, 0-9 y guion)",
                        test.code
                    ),
                ));
            } else if !test_codes.insert(normalized_code.clone()) {
                issues.push(PackIssue::error(
                    "E014",
                    format!("código de ensayo duplicado: «{normalized_code}»"),
                ));
            }

            // E015 · el nombre es obligatorio: `TestDescriptor::new` lo exige, y sin esta
            // regla un nombre vacío hacía desaparecer el ensayo de `descriptors()`.
            if test.name.trim().is_empty() {
                issues.push(PackIssue::error(
                    "E015",
                    format!("el ensayo «{}» no tiene nombre", test.code),
                ));
            }

            // E007 · referencia a norma inexistente
            if test.norm_id.is_some() && resolved.is_none() {
                issues.push(PackIssue::error(
                    "E007",
                    format!(
                        "el ensayo «{}» referencia la norma inexistente «{}»",
                        test.code,
                        test.norm_id.as_deref().unwrap_or("")
                    ),
                ));
            }

            if test.conformity_enabled {
                match resolved {
                    // E006 · norma sin revisar
                    Some(norm) if norm.status != PackStatus::Verified => {
                        issues.push(PackIssue::error(
                            "E006",
                            format!(
                                "el ensayo «{}» habilita conformidad con la norma «{}», que no está revisada",
                                test.code, norm.id
                            ),
                        ));
                    }
                    // E005 · sin límites de especificación
                    Some(norm) if norm.spec_limits.is_none() => {
                        issues.push(PackIssue::error(
                            "E005",
                            format!(
                                "el ensayo «{}» habilita conformidad pero la norma «{}» no define límites (spec_limits es null)",
                                test.code, norm.id
                            ),
                        ));
                    }
                    None if test.norm_id.is_none() => {
                        issues.push(PackIssue::error(
                            "E005",
                            format!(
                                "el ensayo «{}» habilita conformidad sin norma asociada",
                                test.code
                            ),
                        ));
                    }
                    _ => {}
                }
            }

            // W001 · ensayo normativo todavía sin norma: es legítimo como «planificado»
            if test.method_kind == MethodKind::Normative && test.norm_id.is_none() {
                issues.push(PackIssue::warning(
                    "W001",
                    format!(
                        "el ensayo normativo «{}» todavía no tiene norma asociada; no podrá dictaminar",
                        test.code
                    ),
                ));
            }
        }

        issues
    }

    /// Cuántas normas están revisadas.
    pub fn verified_count(&self) -> usize {
        self.norms
            .iter()
            .filter(|n| n.status == PackStatus::Verified)
            .count()
    }
}

// ------------------------------------------------------------- pack cargado ---

/// Un pack que pasó la validación (puede conservar avisos).
#[derive(Clone, Debug)]
pub struct LoadedPack {
    pack: NormPack,
    issues: Vec<PackIssue>,
}

impl LoadedPack {
    /// Carga y valida. Falla con la lista de problemas si hay **cualquier** error;
    /// los avisos no impiden cargar.
    pub fn load(json: &str) -> std::result::Result<Self, Vec<PackIssue>> {
        let pack: NormPack = match serde_json::from_str(json) {
            Ok(pack) => pack,
            Err(error) => {
                return Err(vec![PackIssue::error(
                    "E000",
                    format!("JSON inválido: {error}"),
                )]);
            }
        };
        let issues = pack.validate();
        if issues.iter().any(PackIssue::is_error) {
            return Err(issues);
        }
        Ok(Self { pack, issues })
    }

    pub fn pack(&self) -> &NormPack {
        &self.pack
    }

    pub fn issues(&self) -> &[PackIssue] {
        &self.issues
    }

    pub fn norm(&self, id: &str) -> Option<&PackNorm> {
        self.pack.norm(id)
    }

    /// Referencia normativa resuelta (falla si la entrada es inválida).
    pub fn norm_ref(&self, id: &str) -> Result<NormRef> {
        self.pack
            .norm(id)
            .ok_or_else(|| StandardsError::UnknownNorm(id.to_owned()))?
            .to_norm_ref()
    }

    pub fn tests(&self) -> &[PackTest] {
        &self.pack.tests
    }

    pub fn verified_count(&self) -> usize {
        self.pack.verified_count()
    }

    /// Descriptores de los ensayos declarados. Se omiten los que no se pueden construir
    /// (ya habrían producido un error de validación).
    pub fn descriptors(&self) -> Vec<TestDescriptor> {
        self.pack
            .tests
            .iter()
            .filter_map(|test| {
                let code = TestCode::new(test.code.clone()).ok()?;
                let norm = test
                    .norm_id
                    .as_deref()
                    .and_then(|id| self.norm_ref(id).ok());
                TestDescriptor::new(
                    code,
                    test.name.clone(),
                    test.method_kind,
                    norm,
                    test.conformity_enabled,
                )
                .ok()
            })
            .collect()
    }

    /// Juicio de conformidad para un resultado, usando los límites de la norma del pack.
    ///
    /// Es el único camino previsto para producir un veredicto: pasa por la norma y por
    /// la especificación, y devuelve `NotEvaluated` si falta cualquiera de las dos.
    pub fn evaluate(
        &self,
        test_code: &str,
        value: f64,
        spec: Option<&crate::SpecRef>,
    ) -> crate::Conformity {
        let Some(test) = self.pack.tests.iter().find(|t| t.code == test_code) else {
            return Conformity::NotEvaluated {
                reason: crate::NotEvaluatedReason::NoApplicableSpec,
            };
        };
        let Some(norm_id) = test.norm_id.as_deref() else {
            return Conformity::NotEvaluated {
                reason: crate::NotEvaluatedReason::NoApplicableSpec,
            };
        };
        let Ok(norm) = self.norm_ref(norm_id) else {
            return Conformity::NotEvaluated {
                reason: crate::NotEvaluatedReason::NoApplicableSpec,
            };
        };
        let limits = self.pack.norm(norm_id).and_then(|n| n.spec_limits);
        Conformity::evaluate(value, &norm, spec, limits.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pack mínimo válido, escrito a mano para no depender del pack real.
    fn base_json() -> String {
        r#"{
          "pack_version": 1,
          "pack_id": "test",
          "norms": [{
            "id": "n1",
            "body": "ASTM",
            "designation": "C39/C39M",
            "year": 2026,
            "title": "Compresión",
            "status": "PendingReview",
            "metadata_source": "https://store.astm.org/standards/c39_c39m",
            "metadata_checked_on": "2026-11-20",
            "metadata_confidence": "official",
            "spec_limits": null
          }],
          "tests": [{
            "code": "CONC-COMP-CYL",
            "name": "Compresión de cilindros",
            "norm_id": "n1",
            "method_kind": "Normative",
            "conformity_enabled": false
          }]
        }"#
        .to_owned()
    }

    fn codes(issues: &[PackIssue]) -> Vec<&'static str> {
        issues.iter().map(|i| i.code).collect()
    }

    #[test]
    fn el_pack_minimo_carga() {
        let loaded = LoadedPack::load(&base_json()).expect("debe cargar");
        assert!(loaded.issues().is_empty(), "{:?}", loaded.issues());
        assert_eq!(loaded.verified_count(), 0);
        assert_eq!(loaded.tests().len(), 1);
    }

    #[test]
    fn un_json_roto_no_carga() {
        let issues = LoadedPack::load("{ no es json }").unwrap_err();
        assert_eq!(codes(&issues), vec!["E000"]);
    }

    #[test]
    fn una_version_futura_se_rechaza() {
        let json = base_json().replace("\"pack_version\": 1", "\"pack_version\": 99");
        let issues = LoadedPack::load(&json).unwrap_err();
        assert_eq!(codes(&issues), vec!["E001"]);
    }

    #[test]
    fn revisada_sin_revisor_se_rechaza() {
        let json = base_json().replace("\"status\": \"PendingReview\"", "\"status\": \"Verified\"");
        let issues = LoadedPack::load(&json).unwrap_err();
        assert!(codes(&issues).contains(&"E003"), "{issues:?}");
    }

    #[test]
    fn revisada_sin_fecha_se_rechaza() {
        let json = base_json()
            .replace("\"status\": \"PendingReview\"", "\"status\": \"Verified\"")
            .replace(
                "\"metadata_confidence\": \"official\",",
                "\"metadata_confidence\": \"official\", \"reviewed_by\": \"Ing. Ana\",",
            );
        let issues = LoadedPack::load(&json).unwrap_err();
        assert!(codes(&issues).contains(&"E004"), "{issues:?}");
    }

    #[test]
    fn conformidad_sin_limites_se_rechaza() {
        let json = base_json().replace(
            "\"conformity_enabled\": false",
            "\"conformity_enabled\": true",
        );
        let issues = LoadedPack::load(&json).unwrap_err();
        // Primero falla que la norma no está revisada; con la norma revisada fallaría E005.
        assert!(codes(&issues).contains(&"E006"), "{issues:?}");
    }

    #[test]
    fn conformidad_con_norma_revisada_y_sin_limites_da_e005() {
        let json = base_json()
            .replace("\"status\": \"PendingReview\"", "\"status\": \"Verified\"")
            .replace(
                "\"metadata_confidence\": \"official\",",
                "\"metadata_confidence\": \"official\", \"reviewed_by\": \"Ing. Ana\", \"reviewed_on\": \"2026-11-20\",",
            )
            .replace("\"conformity_enabled\": false", "\"conformity_enabled\": true");
        let issues = LoadedPack::load(&json).unwrap_err();
        assert_eq!(codes(&issues), vec!["E005"], "{issues:?}");
    }

    #[test]
    fn una_norma_inexistente_referenciada_da_e007() {
        let json = base_json().replace("\"norm_id\": \"n1\"", "\"norm_id\": \"n2\"");
        let issues = LoadedPack::load(&json).unwrap_err();
        assert!(codes(&issues).contains(&"E007"), "{issues:?}");
    }

    #[test]
    fn normas_duplicadas_dan_e002() {
        let mut pack: NormPack = serde_json::from_str(&base_json()).unwrap();
        let mut copia = pack.norms[0].clone();
        copia.id = "n2".into();
        pack.norms.push(copia);
        assert!(pack.validate().iter().any(|i| i.code == "E002"));
    }

    #[test]
    fn un_anio_implausible_da_e010() {
        let json = base_json().replace("\"year\": 2026", "\"year\": 3000");
        let issues = LoadedPack::load(&json).unwrap_err();
        assert!(codes(&issues).contains(&"E010"), "{issues:?}");
    }

    #[test]
    fn limites_invertidos_dan_e009() {
        let json = base_json().replace(
            "\"spec_limits\": null",
            "\"spec_limits\": {\"min\": 40.0, \"max\": 20.0}",
        );
        let issues = LoadedPack::load(&json).unwrap_err();
        assert!(codes(&issues).contains(&"E009"), "{issues:?}");
    }

    #[test]
    fn un_ensayo_normativo_sin_norma_avisa_pero_carga() {
        let json = base_json().replace("\"norm_id\": \"n1\",", "");
        let loaded = LoadedPack::load(&json).expect("un aviso no impide cargar");
        assert!(loaded.issues().iter().any(|i| i.code == "W001"));
        assert_eq!(loaded.verified_count(), 0);
    }

    #[test]
    fn un_codigo_de_ensayo_invalido_no_puede_cargarse() {
        // Antes esto cargaba sin avisos y el ensayo desaparecía de `descriptors()`.
        let json = base_json().replace("\"code\": \"CONC-COMP-CYL\"", "\"code\": \"CONC COMP\"");
        let issues = LoadedPack::load(&json).unwrap_err();
        assert_eq!(codes(&issues), vec!["E013"], "{issues:?}");
    }

    #[test]
    fn dos_ensayos_con_el_mismo_codigo_se_rechazan() {
        let mut pack: NormPack = serde_json::from_str(&base_json()).unwrap();
        let mut copia = pack.tests[0].clone();
        copia.name = "Otro nombre".into();
        pack.tests.push(copia);
        let issues = pack.validate();
        assert!(
            issues.iter().any(|i| i.code == "E014"),
            "un código repetido haría ambigua la evaluación: {issues:?}"
        );
    }

    #[test]
    fn un_ensayo_sin_nombre_no_puede_cargarse() {
        let json =
            base_json().replace("\"name\": \"Compresión de cilindros\"", "\"name\": \"   \"");
        let issues = LoadedPack::load(&json).unwrap_err();
        assert_eq!(codes(&issues), vec!["E015"], "{issues:?}");
    }

    #[test]
    fn todos_los_ensayos_del_pack_producen_descriptor() {
        // Ningún ensayo puede desaparecer en silencio: si el pack cargó, todos sus
        // ensayos tienen descriptor.
        let loaded = LoadedPack::load(&base_json()).unwrap();
        assert_eq!(loaded.tests().len(), loaded.descriptors().len());
    }

    #[test]
    fn el_descriptor_se_construye_desde_el_pack() {
        let loaded = LoadedPack::load(&base_json()).unwrap();
        let descriptors = loaded.descriptors();
        assert_eq!(descriptors.len(), 1);
        assert_eq!(descriptors[0].code.as_str(), "CONC-COMP-CYL");
        assert!(!descriptors[0].can_emit_verdict());
    }

    #[test]
    fn evaluate_sin_norma_revisada_no_dictamina() {
        let loaded = LoadedPack::load(&base_json()).unwrap();
        let spec = crate::SpecRef::new("NEC 2015", "f'c").unwrap();
        let c = loaded.evaluate("CONC-COMP-CYL", 28.0, Some(&spec));
        assert_eq!(
            c,
            Conformity::NotEvaluated {
                reason: crate::NotEvaluatedReason::NormNotVerified
            }
        );
    }

    // ---- el pack que se distribuye con el producto ----

    fn shipped_pack() -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/normas/packs/pack-inicial.json"
        );
        std::fs::read_to_string(path).expect("el pack inicial debe existir en el repositorio")
    }

    #[test]
    fn el_pack_inicial_del_repositorio_carga_sin_errores() {
        let loaded = LoadedPack::load(&shipped_pack()).expect("el pack inicial debe ser válido");
        assert!(
            loaded.issues().iter().all(|i| !i.is_error()),
            "{:?}",
            loaded.issues()
        );
    }

    #[test]
    fn el_pack_inicial_no_pretende_estar_verificado() {
        // Si algún día esto falla, es porque alguien marcó una norma como revisada.
        // Eso exige un ingeniero con nombre y fecha: no puede pasar por accidente.
        let loaded = LoadedPack::load(&shipped_pack()).unwrap();
        assert_eq!(
            loaded.verified_count(),
            0,
            "el pack distribuido no puede contener normas verificadas sin revisor real"
        );
        assert!(
            loaded.pack().tests.iter().all(|t| !t.conformity_enabled),
            "ningún ensayo del pack inicial puede dictaminar conformidad"
        );
    }

    #[test]
    fn los_descriptores_del_pack_inicial_se_construyen() {
        let loaded = LoadedPack::load(&shipped_pack()).unwrap();
        let descriptors = loaded.descriptors();
        assert!(!descriptors.is_empty());
        assert!(descriptors.iter().all(|d| !d.can_emit_verdict()));
    }
}
