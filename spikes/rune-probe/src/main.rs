//! MAMUT Lab — Fase 0 risk probe.
//!
//! Proves, on this machine, that the proposed stack works together in one native
//! process:
//!
//! 1. Rune renders a professional shell (sidebar + table + chart + status bar).
//! 2. `rune_kit::Table` gives a virtualised, sortable, resizable technical table.
//! 3. `El::shape` draws a chart from raw vectors (no charting crate needed).
//! 4. `rune::Runtime` drives the app headlessly: deterministic frames, element
//!    rects and simulated input, so UI behaviour is testable in `cargo test`.
//! 5. SQLite (bundled) persists and reads back in the same process.
//! 6. XLSX export writes a real file.
//!
//! IMPORTANT: every number below is a synthetic placeholder used only to fill
//! the layout. None of it is an engineering result and none of it may be reused
//! as test data for the product's calculations.

// `db` and `export` are exercised by the tests below. They stay compiled into the
// binary on purpose: the point of this probe is to prove that SQLite and the XLSX
// writer link into the same native process as the UI.
#![allow(dead_code)]

mod db;
mod export;

use std::borrow::Cow;

use rune::shape::Shape;
use rune::{AppSpec, Cx, El, col, el, row, text};
use rune_kit::{Cell, Kit, Table, TableColumn, TableState};

/// A cylinder specimen row. PROBE DATA ONLY — not an engineering result.
struct Specimen {
    id: String,
    age_days: u32,
    area_mm2: f64,
    load_kn: f64,
    strength_mpa: f64,
}

impl Specimen {
    /// PROBE ONLY. σ = F / A, with F in N and A in mm², so σ is in MPa.
    /// This follows the shape of ASTM C39 but is *not* a validated implementation:
    /// Fase 2 owns the real normative procedure.
    fn strength(load_kn: f64, area_mm2: f64) -> f64 {
        (load_kn * 1000.0) / area_mm2
    }
}

struct State {
    rows: Vec<Specimen>,
    table: TableState,
    active: usize,
    force_dark: Option<bool>,
}

const NAV: [&str; 6] = ["Tablero", "Muestras", "Hormigón", "Suelos", "Equipos", "Informes"];

/// Synthetic rows, deterministic so the probe is reproducible.
fn state() -> State {
    let mut rows = Vec::new();
    for i in 0..240u32 {
        let d = 150.0 + (i as f64 * 37.0) % 55.0;
        let area = std::f64::consts::PI * (d / 2.0) * (d / 2.0);
        let load_kn = 120.0 + (i as f64 * 91.0) % 480.0;
        rows.push(Specimen {
            id: format!("H-2026-{:04}", i + 1),
            age_days: 7 + (i % 6) * 7,
            area_mm2: area,
            load_kn,
            strength_mpa: Specimen::strength(load_kn, area),
        });
    }
    State { rows, table: TableState::new().multi_select(true), active: 2, force_dark: None }
}

fn fmt(n: f64, places: usize) -> String {
    format!("{n:.places$}")
}

/// The strength series as a polyline inside a `w × h` box, plus the box the
/// chart is drawn in. Pure geometry: no rendering, no units, no normative meaning.
fn series_shape(values: &[f64], w: f32, h: f32, color: rune::Color) -> Shape {
    if values.len() < 2 {
        return Shape::new();
    }
    let lo = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (hi - lo).max(1e-9);
    let pad = 8.0_f32;
    let points: Vec<[f32; 2]> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = pad + (i as f32 / (values.len() - 1) as f32) * (w - pad * 2.0);
            let y = pad + (1.0 - ((v - lo) / span) as f32) * (h - pad * 2.0);
            [x, y]
        })
        .collect();
    Shape::new().stroke(points, 1.5, color)
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = s.force_dark.unwrap_or_else(|| cx.system_dark());
    let k = Kit::neutral(dark);
    let accent = rune::tok::accent(0, dark);

    // ---- sidebar -----------------------------------------------------------
    let nav_items: Vec<El> = NAV
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let selected = i == s.active;
            el()
                .key(format!("nav-{i}"))
                .w_full()
                .padding(8.0)
                .radius(k.radius.sm)
                .fill(if selected { k.c.layer_02 } else { k.c.bg_deep })
                .focusable(true)
                .child(text(*name).font(k.ty.text_small).color(if selected { k.c.text_base } else { k.c.text_muted }))
                .on_press(move |s: &mut State| s.active = i)
        })
        .collect();

    let sidebar = col()
        .key("sidebar")
        .w(212.0)
        .h_full()
        .padding(10.0)
        .gap(2.0)
        .fill(k.c.bg_deep)
        .child(
            text("MAMUT Lab")
                .font(k.ty.small)
                .color(k.c.text_base)
                .padding(8.0),
        )
        .children(nav_items);

    // ---- table -------------------------------------------------------------
    let table_k = k.clone();
    let count = s.rows.len();
    let table = Table::new(
        |s: &State| &s.table,
        |s: &mut State| &mut s.table,
        |s: &State| s.rows.len(),
        |s: &State, i| Cow::Borrowed(s.rows[i].id.as_str()),
    )
        .name("Probetas")
        .column(TableColumn::new("id", "Muestra").width(140.0).min(110.0).sortable())
        .column(TableColumn::new("age", "Edad (d)").width(90.0).align(rune::Align::End).sortable())
        .column(TableColumn::new("area", "Área (mm²)").width(110.0).align(rune::Align::End).sortable())
        .column(TableColumn::new("load", "Carga (kN)").width(110.0).align(rune::Align::End).sortable())
        .column(TableColumn::new("strength", "σ (MPa)").flex(1.0).min(110.0).align(rune::Align::End).sortable())
        .cell(move |s: &State, i, col| {
            let r = &s.rows[i];
            match col {
                "id" => Cell::text(&r.id).mono(),
                "age" => Cell::text(r.age_days.to_string()),
                "area" => Cell::text(fmt(r.area_mm2, 1)).mono(),
                "load" => Cell::text(fmt(r.load_kn, 1)).mono(),
                _ => Cell::text(fmt(r.strength_mpa, 2)).mono(),
            }
        })
        .on_sort(|s: &mut State, col, dir, _| {
            let by = |a: &Specimen, b: &Specimen| match col {
                "id" => a.id.cmp(&b.id),
                "age" => a.age_days.cmp(&b.age_days),
                "area" => a.area_mm2.total_cmp(&b.area_mm2),
                "load" => a.load_kn.total_cmp(&b.load_kn),
                _ => a.strength_mpa.total_cmp(&b.strength_mpa),
            };
            s.rows.sort_by(|a, b| dir.apply(by(a, b)));
        })
        .zebra(true)
        .build(s, &table_k)
        .grow(1.0)
        .min_h(0.0);

    // ---- chart -------------------------------------------------------------
    let series: Vec<f64> = s.rows.iter().take(60).map(|r| r.strength_mpa).collect();
    let chart = el()
        .key("chart")
        .w_full()
        .h(150.0)
        .radius(k.radius.md)
        .fill(k.c.bg_base)
        .padding(12.0)
        .shape(series_shape(&series, 900.0, 130.0, accent));

    // ---- KPI strip ---------------------------------------------------------
    let kpi = |label: &'static str, value: String, tint: rune::Color| {
        col()
            .grow(1.0)
            .padding(12.0)
            .gap(4.0)
            .radius(k.radius.md)
            .fill(k.c.bg_base)
            .child(text(label).font(k.ty.text_xsmall).color(k.c.text_muted))
            .child(text(value).font(k.ty.small).color(tint))
    };
    let kpis = row()
        .key("kpis")
        .w_full()
        .gap(8.0)
        .child(kpi("Muestras registradas", count.to_string(), k.c.text_base))
        .child(kpi("Ensayos pendientes", "18".to_string(), k.c.text_base))
        .child(kpi("Fuera de especificación", "3".to_string(), k.c.text_base))
        .child(kpi("Equipos por calibrar", "2".to_string(), k.c.text_base));

    // ---- header / status ---------------------------------------------------
    let header = row()
        .key("header")
        .w_full()
        .padding(12.0)
        .gap(8.0)
        .center_y()
        .fill(k.c.page)
        .child(text(NAV[s.active]).font(k.ty.body).color(k.c.text_base))
        .child(el().grow(1.0).h(1.0))
        .child(
            text(if dark { "Tema oscuro" } else { "Tema claro" })
                .font(k.ty.text_xsmall)
                .color(k.c.text_muted)
                .padding(6.0)
                .radius(k.radius.sm)
                .fill(k.c.layer_01)
                .focusable(true)
                .key("theme")
                .on_press(|s: &mut State| s.force_dark = Some(!s.force_dark.unwrap_or(false))),
        );

    let status = row()
        .key("status")
        .w_full()
        .padding(8.0)
        .fill(k.c.page)
        .child(
            text(format!("{} filas · datos de prueba, no resultados de ensayo", count))
                .font(k.ty.text_xsmall)
                .color(k.c.text_faint),
        );

    let main = col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .gap(10.0)
        .padding(12.0)
        .fill(k.c.page)
        .child(header)
        .child(kpis)
        .child(
            col()
                .grow(1.0)
                .min_h(0.0)
                .key("table-frame")
                .radius(k.radius.md)
                .clip_rounded(k.radius.md)
                .fill(k.c.bg_base)
                .child(table),
        )
        .child(chart)
        .child(status);

    row().w_full().h_full().fill(k.c.bg_deep).child(sidebar).child(main)
}

fn main() {
    rune::run(AppSpec::new("MAMUT Lab — probe", (1180.0, 800.0), state, view));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    fn runtime() -> Runtime<State> {
        let env = Env { system_dark: true, scale: 2.0 };
        let mut rt = Runtime::new(state(), view, (1180.0, 800.0), env);
        rt.frame(0.0);
        rt
    }

    /// The shell lays out: sidebar, table and chart all have presented rects.
    #[test]
    fn shell_lays_out() {
        let rt = runtime();
        assert!(rt.presented("sidebar").is_some(), "sidebar is laid out");
        assert!(rt.presented("table-frame").is_some(), "table frame is laid out");
        assert!(rt.presented("chart").is_some(), "chart is laid out");
        assert_eq!(rt.state.rows.len(), 240);
    }

    /// Simulated input drives real state: pressing a nav item selects it.
    #[test]
    fn nav_press_changes_state() {
        let mut rt = runtime();
        assert_eq!(rt.state.active, 2);
        let (x, y) = rt.find("nav-4").expect("nav item 4 exists");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.active, 4, "pressing Suelos selects it");
    }

    /// The chart geometry is pure and testable without a GPU.
    #[test]
    fn chart_geometry_maps_values_into_the_box() {
        let shape = series_shape(&[10.0, 20.0, 15.0], 100.0, 100.0, rune::Color::WHITE);
        assert!(!shape.is_empty());
        // Degenerate inputs must not panic.
        assert!(series_shape(&[1.0], 100.0, 100.0, rune::Color::WHITE).is_empty());
        assert!(series_shape(&[5.0, 5.0], 100.0, 100.0, rune::Color::WHITE).is_empty() == false);
    }

    /// SQLite works in-process: the offline-first storage path.
    #[test]
    fn sqlite_round_trip() {
        let conn = db::open_in_memory().expect("open db");
        db::migrate(&conn).expect("migrate");
        db::insert_sample(&conn, "H-2026-0001", "hormigón", "2026-11-18").expect("insert");
        let found = db::get_sample(&conn, "H-2026-0001").expect("query");
        assert_eq!(found.as_deref(), Some("hormigón"));
        assert!(db::get_sample(&conn, "missing").expect("query").is_none());
    }

    /// XLSX export produces a real file on disk.
    #[test]
    fn xlsx_export_writes_a_file() {
        let dir = std::env::temp_dir().join(format!("mamut-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("probe.xlsx");
        export::write_xlsx(&path, &[("H-2026-0001", 24.5), ("H-2026-0002", 31.2)]).expect("write xlsx");
        let len = std::fs::metadata(&path).unwrap().len();
        assert!(len > 1000, "xlsx is a real zip container, got {len} bytes");
        std::fs::remove_dir_all(&dir).ok();
    }
}
