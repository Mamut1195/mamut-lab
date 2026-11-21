//! Carga del *norm pack*.
//!
//! El pack **no** se compila dentro de la lógica: se busca primero en la carpeta de datos
//! del usuario (`norm-packs/*.json`) y, si no hay ninguno válido, se usa el que viaja
//! embebido en el binario. Así un laboratorio puede incorporar su pack verificado sin
//! esperar una versión nueva del programa, que es lo que promete ADR-0009.
//!
//! Un pack del usuario que no carga **no se ignora en silencio**: se usa el embebido para
//! que la aplicación siga siendo útil, pero los problemas se devuelven en
//! [`PackSelection::issues`] y acaban en la barra de estado.

use std::path::{Path, PathBuf};

use mamut_standards::{LoadedPack, PackIssue};

/// Pack que viaja con el binario, para que la aplicación funcione recién instalada y sin
/// conexión.
pub const EMBEDDED_PACK: &str = include_str!("../../../docs/normas/packs/pack-inicial.json");

/// Subcarpeta de datos donde se buscan packs del usuario.
pub const PACK_DIR: &str = "norm-packs";

/// Pack elegido, de dónde salió y qué problemas se encontraron por el camino.
pub struct PackSelection {
    pub pack: LoadedPack,
    /// `embebido` o `archivo: <nombre>`.
    pub source: String,
    /// Problemas de packs del usuario descartados (vacío si todo fue bien).
    pub issues: Vec<PackIssue>,
}

impl PackSelection {
    /// Si hubo que descartar el pack del usuario.
    pub fn has_problems(&self) -> bool {
        !self.issues.is_empty()
    }
}

/// Elige el pack a usar.
///
/// Solo devuelve `Err` si el **pack embebido** no es válido, que sería un defecto de la
/// compilación: un pack del usuario defectuoso se informa en `issues`.
pub fn load_pack(data_dir: Option<&Path>) -> Result<PackSelection, Vec<PackIssue>> {
    let (found, issues) = match data_dir {
        Some(dir) => load_from_dir(&dir.join(PACK_DIR)),
        None => (None, Vec::new()),
    };

    if let Some((pack, source)) = found {
        return Ok(PackSelection {
            pack,
            source,
            issues,
        });
    }

    let pack = LoadedPack::load(EMBEDDED_PACK)?;
    Ok(PackSelection {
        pack,
        source: "embebido".to_owned(),
        issues,
    })
}

/// Devuelve el primer pack válido y la lista de problemas de los que no lo eran.
fn load_from_dir(pack_dir: &Path) -> (Option<(LoadedPack, String)>, Vec<PackIssue>) {
    let Ok(entries) = std::fs::read_dir(pack_dir) else {
        return (None, Vec::new());
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        })
        .collect();
    paths.sort();

    let mut issues = Vec::new();
    let mut valid: Vec<(LoadedPack, String)> = Vec::new();

    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        let Ok(text) = std::fs::read_to_string(&path) else {
            issues.push(PackIssue::error(
                "E000",
                format!("no se pudo leer el pack «{name}»"),
            ));
            continue;
        };

        match LoadedPack::load(&text) {
            Ok(pack) => valid.push((pack, name)),
            Err(mut problems) => {
                for problem in &mut problems {
                    problem.message = format!("«{name}»: {}", problem.message);
                }
                issues.extend(problems);
            }
        }
    }

    if valid.is_empty() {
        return (None, issues);
    }

    if valid.len() > 1 {
        let ignored: Vec<&str> = valid
            .iter()
            .skip(1)
            .map(|(_, name)| name.as_str())
            .collect();
        issues.push(PackIssue::warning(
            "W101",
            format!(
                "hay {} packs válidos; se usa «{}» y se ignoran: {}",
                valid.len(),
                valid[0].1,
                ignored.join(", ")
            ),
        ));
    }

    let (pack, name) = valid.remove(0);
    (Some((pack, format!("archivo: {name}"))), issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mamut-pack-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sin_carpeta_se_usa_el_pack_embebido() {
        let selection = load_pack(None).expect("el pack embebido debe ser válido");
        assert_eq!(selection.source, "embebido");
        assert_eq!(selection.pack.verified_count(), 0);
        assert!(!selection.has_problems());
    }

    #[test]
    fn una_carpeta_vacia_cae_al_pack_embebido() {
        let dir = temp_dir("empty");
        let selection = load_pack(Some(&dir)).unwrap();
        assert_eq!(selection.source, "embebido");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_pack_del_usuario_se_carga_sin_recompilar() {
        let dir = temp_dir("user");
        let packs = dir.join(PACK_DIR);
        std::fs::create_dir_all(&packs).unwrap();

        let custom = r#"{
          "pack_version": 1,
          "pack_id": "laboratorio-central",
          "norms": [{
            "id": "propia-1",
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
          "tests": []
        }"#;
        std::fs::write(packs.join("propio.json"), custom).unwrap();

        let selection = load_pack(Some(&dir)).unwrap();
        assert_eq!(selection.source, "archivo: propio.json");
        assert_eq!(selection.pack.pack().pack_id, "laboratorio-central");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_pack_del_usuario_invalido_se_informa_y_se_usa_el_embebido() {
        let dir = temp_dir("invalid");
        let packs = dir.join(PACK_DIR);
        std::fs::create_dir_all(&packs).unwrap();
        std::fs::write(packs.join("roto.json"), "{ no es json }").unwrap();

        let selection = load_pack(Some(&dir)).unwrap();
        assert_eq!(
            selection.source, "embebido",
            "la aplicación sigue arrancando"
        );
        assert!(
            selection.has_problems(),
            "el fallo debe llegar al usuario, no quedar tapado"
        );
        assert!(
            selection.issues.iter().any(|issue| issue.code == "E000"),
            "{:?}",
            selection.issues
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_pack_valido_gana_a_uno_roto() {
        let dir = temp_dir("mixed");
        let packs = dir.join(PACK_DIR);
        std::fs::create_dir_all(&packs).unwrap();
        std::fs::write(packs.join("a-roto.json"), "{ roto }").unwrap();
        std::fs::write(packs.join("b-bueno.json"), EMBEDDED_PACK).unwrap();

        let selection = load_pack(Some(&dir)).unwrap();
        assert_eq!(selection.source, "archivo: b-bueno.json");
        assert!(selection.has_problems(), "el roto se reporta igualmente");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dos_packs_validos_avisan_de_cual_se_usa() {
        let dir = temp_dir("two");
        let packs = dir.join(PACK_DIR);
        std::fs::create_dir_all(&packs).unwrap();
        std::fs::write(packs.join("a.json"), EMBEDDED_PACK).unwrap();
        std::fs::write(packs.join("b.json"), EMBEDDED_PACK).unwrap();

        let selection = load_pack(Some(&dir)).unwrap();
        assert_eq!(selection.source, "archivo: a.json", "orden alfabético");
        assert!(
            selection.issues.iter().any(|issue| issue.code == "W101"),
            "elegir uno y callar los demás ocultaría un conflicto de configuración"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
