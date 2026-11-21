#![allow(clippy::unwrap_used, clippy::expect_used)]
// Las pruebas usan unwrap/expect a propósito: si una precondición falla,
// queremos que la prueba estalle justo ahí.

//! Pruebas de arquitectura.
//!
//! Las reglas de dependencia del ADR-0003 y del ADR-0008 son **ejecutables**. Sin esta
//! prueba serían una intención escrita en un documento que nadie relee.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Cargo.toml de un crate del workspace.
fn manifest(crate_name: &str) -> String {
    let path = repo_root()
        .join("crates")
        .join(crate_name)
        .join("Cargo.toml");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("no se pudo leer {path:?}: {error}"))
}

/// Quita los comentarios: un crate puede *mencionar* Rune en un comentario sin depender
/// de él, y eso no debe fallar la prueba.
fn without_comments(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

fn crate_dirs() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let root = repo_root().join("crates");
    let entries = fs::read_dir(&root).unwrap_or_else(|e| panic!("no se pudo leer {root:?}: {e}"));
    for entry in entries {
        let dir = entry.expect("entrada legible").path();
        if !dir.is_dir() {
            continue;
        }
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Ok(text) = fs::read_to_string(dir.join("Cargo.toml")) {
            out.push((name, without_comments(&text)));
        }
    }
    out
}

#[test]
fn estan_los_doce_crates_del_diseno() {
    let crates = crate_dirs();
    let names: Vec<&str> = crates.iter().map(|(name, _)| name.as_str()).collect();
    for expected in [
        "mamut-core",
        "mamut-standards",
        "mamut-domain",
        "mamut-calculations",
        "mamut-validation",
        "mamut-charts",
        "mamut-storage",
        "mamut-reports",
        "mamut-equipment",
        "mamut-import",
        "mamut-export",
        "mamut-app",
    ] {
        assert!(
            names.contains(&expected),
            "falta el crate «{expected}» del ADR-0003; hay: {names:?}"
        );
    }
    assert_eq!(
        crates.len(),
        12,
        "el diseño tiene doce crates, hay {names:?}"
    );
}

#[test]
fn solo_la_capa_de_aplicacion_depende_de_rune() {
    let mut checked = 0;
    for (name, manifest) in crate_dirs() {
        // `package = "rune"` cubre el caso de una dependencia renombrada, que de otro modo
        // se colaría como si fuera otra cosa.
        let uses_rune = manifest.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("rune")
                || line.contains("package = \"rune\"")
                || line.contains("package = \"rune-kit\"")
        });

        if name == "mamut-app" {
            assert!(uses_rune, "mamut-app debe depender de Rune: es su trabajo");
        } else {
            assert!(
                !uses_rune,
                "«{name}» no puede depender de Rune: la interfaz debe ser reemplazable (ADR-0008)"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 12);
}

#[test]
fn la_app_depende_de_rune_kit_y_no_al_reves() {
    let app = manifest("mamut-app");
    assert!(
        app.contains("rune-kit"),
        "la app usa los componentes de rune-kit"
    );
}

#[test]
fn el_dominio_no_depende_de_infraestructura() {
    let manifest = manifest("mamut-domain");
    for forbidden in ["mamut-storage", "mamut-app", "rusqlite", "rune"] {
        assert!(
            !manifest.contains(forbidden),
            "mamut-domain no puede depender de {forbidden} (ADR-0003)"
        );
    }
}

#[test]
fn los_calculos_no_dependen_de_persistencia_ni_de_interfaz() {
    let manifest = manifest("mamut-calculations");
    for forbidden in ["mamut-storage", "mamut-app", "rusqlite", "rune"] {
        assert!(
            !manifest.contains(forbidden),
            "mamut-calculations no puede depender de {forbidden}: los algoritmos son puros"
        );
    }
}

/// Sección `[dependencies]` de un manifiesto, sin las demás tablas.
fn dependencies_section(manifest: &str) -> String {
    manifest
        .split("[dependencies]")
        .nth(1)
        .unwrap_or("")
        .split("\n[")
        .next()
        .unwrap_or("")
        .to_owned()
}

#[test]
fn las_normas_solo_dependen_de_core_y_de_serde() {
    let manifest = manifest("mamut-standards");
    let deps = dependencies_section(&manifest);
    for line in deps.lines() {
        let line = line.trim_start();
        if line.starts_with("mamut-") {
            assert!(
                line.starts_with("mamut-core"),
                "mamut-standards solo puede depender de mamut-core del producto, encontró: {line}"
            );
        }
    }
}

#[test]
fn los_graficos_no_tienen_dependencias() {
    let manifest = manifest("mamut-charts");
    let deps_section = manifest
        .split("[dependencies]")
        .nth(1)
        .unwrap_or("")
        .split('[')
        .next()
        .unwrap_or("");
    assert!(
        deps_section.trim().is_empty(),
        "mamut-charts debe ser geometría pura, sin dependencias: {deps_section}"
    );
}

#[test]
fn la_auditoria_y_el_almacenamiento_viven_fuera_del_dominio() {
    let storage = manifest("mamut-storage");
    assert!(
        storage.contains("mamut-domain"),
        "storage implementa los puertos del dominio"
    );
    assert!(
        storage.contains("rusqlite"),
        "storage es quien habla con SQLite"
    );
}

#[test]
fn rune_esta_fijado_a_una_revision_y_no_a_una_rama() {
    let root = fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml raíz");
    let clean = without_comments(&root);

    assert!(
        clean.contains("rev = \""),
        "Rune debe fijarse por revisión, no por rama"
    );
    assert!(
        !clean.contains("branch = \"dev\""),
        "fijar la rama `dev` haría la compilación no reproducible"
    );
}

#[test]
fn rune_y_sus_parches_comparten_la_misma_revision() {
    // Hallazgo H-1 de la Fase 0: los cinco parches deben moverse con Rune o la
    // resolución de dependencias falla. Esta prueba impide que se desincronicen.
    let root = fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml raíz");
    let clean = without_comments(&root);

    let mut revisions = Vec::new();
    let mut rest = clean.as_str();
    while let Some(index) = rest.find("rev = \"") {
        let after = &rest[index + 7..];
        let end = after.find('"').unwrap_or(0);
        revisions.push(after[..end].to_owned());
        rest = &after[end..];
    }

    assert!(
        revisions.len() >= 6,
        "se esperaban Rune + 5 parches, se encontraron {} revisiones: {revisions:?}",
        revisions.len()
    );
    let first = revisions[0].clone();
    assert!(
        revisions.iter().all(|rev| *rev == first),
        "todas las revisiones deben coincidir; hay: {revisions:?}"
    );
    assert!(
        first.len() == 40,
        "una revisión de git es un SHA de 40 caracteres: «{first}»"
    );
}
