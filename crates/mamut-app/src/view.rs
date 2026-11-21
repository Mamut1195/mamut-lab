//! La vista: una función del estado, sin efectos.
//!
//! No consulta la base de datos, no calcula y no decide conformidad: presenta lo que el
//! estado ya contiene y delega las acciones en sus métodos. Todo lo que se ve aquí se
//! puede probar sin abrir una ventana.

use mamut_charts::{LineChartSpec, Rect, Scale, line_chart};
use mamut_domain::Material;
use rune::shape::Shape;
use rune::{Cx, El, Key, KeyEvent, ScrollSpec, Tx, col, el, row, text};
use rune_kit::{Cell, Kit, SortDir, Table, TableColumn, TextField};

use crate::state::{AppState, Nav, SampleRow};
use crate::store::ExportFormat;

/// Serie resistencia–edad como polilínea, para el gráfico del ensayo.
///
/// La geometría la calcula `mamut-charts` (probada sin ventana); aquí solo se convierte
/// en un trazo de Rune.
fn series_shape(series: &[(u32, f64)], width: f32, height: f32, color: rune::Color) -> Shape {
    if series.len() < 2 {
        return Shape::new();
    }
    let points: Vec<(f64, f64)> = series
        .iter()
        .map(|(age, stress)| (f64::from(*age), *stress))
        .collect();
    let x_min = points[0].0;
    let x_max = points[points.len() - 1].0;
    let y_min = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let y_max = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);

    if x_max <= x_min || y_max <= y_min {
        return Shape::new();
    }

    let spec = LineChartSpec {
        points: &points,
        plot: Rect::new(0.0, 0.0, width, height),
        x: Scale::Linear {
            min: x_min,
            max: x_max,
        },
        y: Scale::Linear {
            min: y_min,
            max: y_max,
        },
        x_ticks: Vec::new(),
        y_ticks: Vec::new(),
        limits: Vec::new(),
    };
    Shape::new().stroke(line_chart(&spec).line, 1.5, color)
}

/// Atajos de teclado del armazón.
///
/// Devuelve `true` si la tecla quedó atendida, que es lo que Rune espera del manejador.
pub fn handle_key(state: &mut AppState, key: &KeyEvent) -> bool {
    if key.is_command('k') {
        state.toggle_palette();
        return true;
    }

    if matches!(&key.key, Key::Escape) {
        if state.form_open {
            state.cancel_form();
            return true;
        }
        if state.palette_open {
            state.close_palette();
            return true;
        }
        return false;
    }

    const DIGITS: [char; 6] = ['1', '2', '3', '4', '5', '6'];
    for (index, digit) in DIGITS.iter().enumerate() {
        if key.is_command(*digit) {
            if let Some(target) = Nav::from_index(index) {
                state.select(target);
            }
            return true;
        }
    }

    false
}

/// Manejador de teclado a nivel de ventana.
///
/// Rune entrega las teclas a la ventana cuando ningún campo tiene el foco (ver
/// `AppSpec::on_key`). Un `.on_key` en un elemento solo se dispara si ese elemento tiene
/// el foco, así que los atajos globales van aquí; así funcionan siempre.
pub fn window_key(state: &mut AppState, key: &KeyEvent, _tx: &mut Tx) -> bool {
    handle_key(state, key)
}

fn sort_rows(state: &mut AppState, column: &str, direction: SortDir) {
    let compare = |a: &SampleRow, b: &SampleRow| match column {
        "material" => a.material.cmp(&b.material),
        "client" => a.client.cmp(&b.client),
        "received" => a.received_on.cmp(&b.received_on),
        "state" => a.state.cmp(&b.state),
        _ => a.code.cmp(&b.code),
    };
    state.rows.sort_by(|a, b| direction.apply(compare(a, b)));
}

pub fn view(state: &AppState, cx: &mut Cx<AppState>) -> El {
    state.sync_system_theme(cx.system_dark());
    let dark = state.is_dark(cx.system_dark());
    let kit = Kit::neutral(dark);
    let table_kit = kit.clone();

    // ------------------------------------------------------------ navegación
    let nav_items: Vec<El> = Nav::ALL
        .iter()
        .enumerate()
        .map(|(index, nav)| {
            let selected = *nav == state.nav;
            el().key(format!("nav-{index}"))
                .w_full()
                .padding(8.0)
                .radius(kit.radius.sm)
                .fill(if selected {
                    kit.c.layer_02
                } else {
                    kit.c.bg_deep
                })
                .focusable(true)
                .child(
                    text(nav.label())
                        .font(kit.ty.text_small)
                        .color(if selected {
                            kit.c.text_base
                        } else {
                            kit.c.text_muted
                        }),
                )
                .on_press(move |s: &mut AppState| {
                    if let Some(target) = Nav::from_index(index) {
                        s.select(target);
                    }
                })
        })
        .collect();

    let sidebar = col()
        .key("sidebar")
        .w(216.0)
        .h_full()
        .padding(10.0)
        .gap(2.0)
        .fill(kit.c.bg_deep)
        .child(
            text("MAMUT Lab")
                .font(kit.ty.small)
                .color(kit.c.text_base)
                .padding(8.0),
        )
        .children(nav_items)
        .child(
            text(format!("Normas: {} verificadas", state.norms_verified))
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_faint)
                .padding(8.0),
        );

    // ---------------------------------------------------------------- cabecera
    let mut header = row()
        .key("header")
        .w_full()
        .padding(12.0)
        .gap(8.0)
        .center_y()
        .fill(kit.c.page)
        .child(
            text(state.nav.label())
                .font(kit.ty.body)
                .color(kit.c.text_base),
        );

    if state.nav == Nav::Samples {
        header = header.child(
            el().key("new-sample")
                .padding(6.0)
                .radius(kit.radius.sm)
                .fill(kit.c.layer_01)
                .focusable(true)
                .child(
                    text("+ Nueva muestra")
                        .font(kit.ty.text_xsmall)
                        .color(kit.c.text_base),
                )
                .on_press(|s: &mut AppState| s.open_form()),
        );
    }

    let header = header.child(el().grow(1.0).h(1.0)).child(
        text(if dark { "Tema: oscuro" } else { "Tema: claro" })
            .font(kit.ty.text_xsmall)
            .color(kit.c.text_muted)
            .padding(6.0)
            .radius(kit.radius.sm)
            .fill(kit.c.layer_01)
            .focusable(true)
            .key("theme")
            .on_press(|s: &mut AppState| s.toggle_theme()),
    );

    // ------------------------------------------------------------------- KPIs
    let kpi = |label: &'static str, value: String| {
        col()
            .grow(1.0)
            .padding(12.0)
            .gap(4.0)
            .radius(kit.radius.md)
            .fill(kit.c.bg_base)
            .child(text(label).font(kit.ty.text_xsmall).color(kit.c.text_muted))
            .child(text(value).font(kit.ty.small).color(kit.c.text_base))
    };

    let kpis = row()
        .key("kpis")
        .w_full()
        .gap(8.0)
        .child(kpi("Muestras registradas", state.samples_total.to_string()))
        .child(kpi("Ensayos registrados", state.tests_total.to_string()))
        .child(kpi("Normas verificadas", state.norms_verified.to_string()))
        .child(kpi("Normas pendientes", state.norms_pending.to_string()));

    // ------------------------------------------------------------------ aviso
    let flash: Option<El> = state.flash.as_ref().map(|message| {
        text(message.clone())
            .key("flash")
            .font(kit.ty.text_xsmall)
            .color(kit.c.text_muted)
            .padding(4.0)
    });

    // ------------------------------------------------------- alta de muestras
    let field = |label: &'static str,
                 placeholder: &'static str,
                 edit: &rune::TextEdit,
                 on_edit: fn(&mut AppState) -> &mut rune::TextEdit| {
        TextField::new(label, edit)
            .placeholder(placeholder)
            .on_edit(on_edit)
            .build(&kit)
            .grow(1.0)
            .min_w(0.0)
    };

    let form_error: Option<El> = state.form.error.as_ref().map(|message| {
        text(format!("⚠ {message}"))
            .key("form-error")
            .font(kit.ty.text_xsmall)
            .color(kit.c.text_base)
            .padding(4.0)
    });

    let materials: Vec<El> = [
        Material::Concrete,
        Material::Soil,
        Material::Aggregate,
        Material::Asphalt,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, material)| {
        let selected = state.form.material == material;
        el().key(format!("material-{index}"))
            .padding(6.0)
            .radius(kit.radius.sm)
            .fill(if selected {
                kit.c.layer_02
            } else {
                kit.c.bg_deep
            })
            .focusable(true)
            .child(
                text(material.label())
                    .font(kit.ty.text_xsmall)
                    .color(if selected {
                        kit.c.text_base
                    } else {
                        kit.c.text_muted
                    }),
            )
            .on_press(move |s: &mut AppState| s.set_material(material))
    })
    .collect();

    // El formulario ocupa el área de trabajo mientras está abierto: así no compite por el
    // alto con la tabla y sus botones siempre quedan accesibles.
    let form_panel: El = {
        col()
            .key("sample-form")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .padding(12.0)
            .gap(8.0)
            .radius(kit.radius.md)
            .fill(kit.c.bg_base)
            .child(
                text("Nueva muestra")
                    .font(kit.ty.small)
                    .color(kit.c.text_base),
            )
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .child(field(
                        "Código",
                        "H-2026-0001",
                        &state.form.code,
                        |s: &mut AppState| &mut s.form.code,
                    ))
                    .child(field(
                        "Procedencia",
                        "Bloque 3, planta 5",
                        &state.form.origin,
                        |s: &mut AppState| &mut s.form.origin,
                    ))
                    .child(field(
                        "Cliente",
                        "Constructora Andina",
                        &state.form.client,
                        |s: &mut AppState| &mut s.form.client,
                    )),
            )
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .child(field(
                        "Proyecto",
                        "Torre Norte",
                        &state.form.project,
                        |s: &mut AppState| &mut s.form.project,
                    ))
                    .child(field(
                        "Recepción (AAAA-MM-DD)",
                        "2026-11-20",
                        &state.form.received_on,
                        |s: &mut AppState| &mut s.form.received_on,
                    ))
                    .child(field(
                        "Responsable",
                        "Ana Torres",
                        &state.form.responsible,
                        |s: &mut AppState| &mut s.form.responsible,
                    )),
            )
            .child(
                row()
                    .w_full()
                    .gap(6.0)
                    .center_y()
                    .child(
                        text("Material")
                            .font(kit.ty.text_xsmall)
                            .color(kit.c.text_muted)
                            .padding(4.0),
                    )
                    .children(materials),
            )
            .children(form_error)
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .child(
                        el().key("save-sample")
                            .padding(8.0)
                            .radius(kit.radius.sm)
                            .fill(kit.c.layer_02)
                            .focusable(true)
                            .child(
                                text("Guardar")
                                    .font(kit.ty.text_small)
                                    .color(kit.c.text_base),
                            )
                            .on_press(|s: &mut AppState| s.save_form()),
                    )
                    .child(
                        el().key("cancel-sample")
                            .padding(8.0)
                            .radius(kit.radius.sm)
                            .fill(kit.c.bg_deep)
                            .focusable(true)
                            .child(
                                text("Cancelar")
                                    .font(kit.ty.text_small)
                                    .color(kit.c.text_muted),
                            )
                            .on_press(|s: &mut AppState| s.cancel_form()),
                    ),
            )
    };

    // ------------------------------------------------- ensayo de compresión
    let action = |key: &'static str, label: &'static str, run: fn(&mut AppState)| {
        el().key(key)
            .padding(8.0)
            .radius(kit.radius.sm)
            .fill(kit.c.layer_02)
            .focusable(true)
            .child(text(label).font(kit.ty.text_small).color(kit.c.text_base))
            .on_press(move |s: &mut AppState| run(s))
    };

    let test_error: Option<El> = state.test_form.error.as_ref().map(|message| {
        text(format!("⚠ {message}"))
            .key("test-error")
            .font(kit.ty.text_xsmall)
            .color(kit.c.text_base)
            .padding(4.0)
    });

    let test_warnings: Option<El> =
        (!state.test_form.warnings.is_empty()).then(|| {
            col().key("test-warnings").w_full().gap(2.0).children(
                state.test_form.warnings.iter().map(|warning| {
                    text(format!("⚠ {warning}"))
                        .font(kit.ty.text_xsmall)
                        .color(kit.c.text_muted)
                }),
            )
        });

    let test_result: Option<El> = state.test_form.outcome.as_ref().map(|outcome| {
        col()
            .key("test-result")
            .w_full()
            .padding(10.0)
            .gap(3.0)
            .radius(kit.radius.sm)
            .fill(kit.c.bg_deep)
            .child(
                text(format!("Resistencia: {:.2} MPa", outcome.stress_display))
                    .font(kit.ty.small)
                    .color(kit.c.text_base),
            )
            .child(
                text(format!(
                    "Área {:.2} mm² · esbeltez {:.2} · {}",
                    outcome.area_mm2, outcome.slenderness, outcome.norm_citation
                ))
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_muted),
            )
            .child(
                text(outcome.conformity.clone())
                    .font(kit.ty.text_xsmall)
                    .color(kit.c.text_muted),
            )
    });

    let report_line: Option<El> = state.test_form.report_path.as_ref().map(|path| {
        text(format!("Informe generado en {path}"))
            .key("report-path")
            .font(kit.ty.text_xsmall)
            .color(kit.c.text_muted)
            .padding(4.0)
    });

    let test_history: Option<El> = (!state.test_history.is_empty()).then(|| {
        col()
            .key("test-history")
            .w_full()
            .gap(2.0)
            .child(
                text("Historial de la muestra")
                    .font(kit.ty.text_xsmall)
                    .color(kit.c.text_muted),
            )
            .children(state.test_history.iter().map(|row| {
                text(format!(
                    "{} · {} · {} · {}",
                    row.performed_on, row.designation, row.stress, row.state
                ))
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_faint)
            }))
    });

    let chart: Option<El> = (state.series.len() >= 2).then(|| {
        el().key("series-chart")
            .w_full()
            .h(120.0)
            .padding(8.0)
            .radius(kit.radius.sm)
            .fill(kit.c.bg_deep)
            .shape(series_shape(
                &state.series,
                900.0,
                110.0,
                rune::tok::accent(0, dark),
            ))
    });

    let test_panel: El = col()
        .key("test-panel")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .padding(12.0)
        .gap(8.0)
        .radius(kit.radius.md)
        .fill(kit.c.bg_base)
        .child(
            text("Compresión de cilindros")
                .font(kit.ty.small)
                .color(kit.c.text_base),
        )
        .child(
            text(
                "Escribe el código de una muestra ya registrada: el ensayo se guarda a su nombre.",
            )
            .font(kit.ty.text_xsmall)
            .color(kit.c.text_faint),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(field(
                    "Muestra",
                    "H-2026-0001",
                    &state.test_form.sample_code,
                    |s: &mut AppState| &mut s.test_form.sample_code,
                ))
                .child(field(
                    "Designación",
                    "C1",
                    &state.test_form.designation,
                    |s: &mut AppState| &mut s.test_form.designation,
                ))
                .child(field(
                    "Edad (días)",
                    "28",
                    &state.test_form.age,
                    |s: &mut AppState| &mut s.test_form.age,
                )),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(field(
                    "Diámetro (mm)",
                    "150",
                    &state.test_form.diameter,
                    |s: &mut AppState| &mut s.test_form.diameter,
                ))
                .child(field(
                    "Altura (mm)",
                    "300",
                    &state.test_form.height,
                    |s: &mut AppState| &mut s.test_form.height,
                ))
                .child(field(
                    "Masa (g)",
                    "12300",
                    &state.test_form.mass,
                    |s: &mut AppState| &mut s.test_form.mass,
                )),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(field(
                    "Carga máxima (kN)",
                    "500",
                    &state.test_form.load,
                    |s: &mut AppState| &mut s.test_form.load,
                ))
                .child(action("calculate-test", "Calcular", |s: &mut AppState| {
                    s.calculate_test()
                }))
                .child(action("save-test", "Guardar ensayo", |s: &mut AppState| {
                    s.save_test()
                }))
                .child(action("report-pdf", "Informe PDF", |s: &mut AppState| {
                    s.generate_report()
                })),
        )
        .children(test_error)
        .children(test_warnings)
        .children(test_result)
        .children(report_line)
        .children(test_history)
        .children(chart);

    // ----------------------------------------------------- granulometría suelos
    let sieve_rows: Vec<El> = (0..state.soils.rows.len())
        .map(|index| {
            row()
                .w_full()
                .gap(8.0)
                .child(
                    TextField::new("Tamiz (mm)", &state.soils.rows[index].size)
                        .placeholder("abertura")
                        .on_edit(move |s: &mut AppState| &mut s.soils.rows[index].size)
                        .build(&kit)
                        .grow(1.0)
                        .min_w(0.0),
                )
                .child(
                    TextField::new("Retenido (g)", &state.soils.rows[index].retained)
                        .placeholder("masa")
                        .on_edit(move |s: &mut AppState| &mut s.soils.rows[index].retained)
                        .build(&kit)
                        .grow(1.0)
                        .min_w(0.0),
                )
        })
        .collect();

    let soils_error: Option<El> = state.soils.error.as_ref().map(|message| {
        text(format!("⚠ {message}"))
            .key("soils-error")
            .font(kit.ty.text_xsmall)
            .color(kit.c.text_base)
            .padding(4.0)
    });

    let soils_result: Option<El> = state.soils.outcome.as_ref().map(|outcome| {
        let mut column = col().key("soils-result").w_full().gap(2.0).padding(10.0);
        if let (Some(cu), Some(cc)) = (outcome.cu, outcome.cc) {
            column = column.child(
                text(format!("Cu {cu:.2} · Cc {cc:.2}"))
                    .font(kit.ty.small)
                    .color(kit.c.text_base),
            );
        }
        let mut summary = String::from("Diámetros:");
        for (label, value) in [
            ("D10", outcome.d10),
            ("D30", outcome.d30),
            ("D60", outcome.d60),
        ] {
            match value {
                Some(value) => summary.push_str(&format!(" {label} {value:.3} mm")),
                None => summary.push_str(&format!(" {label} —")),
            }
        }
        column = column.child(
            text(summary)
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_muted),
        );
        column = column.child(
            text(format!(
                "{} tamices · {}",
                outcome.sieve_count,
                if outcome.d_at_boundary {
                    "algún diámetro cae en el borde de la curva: no hay intervalo de interpolación"
                } else {
                    "diámetros interpolados entre tamices"
                }
            ))
            .font(kit.ty.text_xsmall)
            .color(kit.c.text_faint),
        );
        let lines: Vec<El> = outcome
            .passing
            .iter()
            .take(12)
            .map(|(size, passing)| {
                text(format!("{size} mm · pasa {passing:.1} %"))
                    .font(kit.ty.text_xsmall)
                    .color(kit.c.text_faint)
            })
            .collect();
        column.children(lines)
    });

    let limit_rows: Vec<El> = (0..state.limits.rows.len())
        .map(|index| {
            row()
                .w_full()
                .gap(8.0)
                .child(
                    TextField::new("Golpes", &state.limits.rows[index].blows)
                        .placeholder("N")
                        .on_edit(move |s: &mut AppState| &mut s.limits.rows[index].blows)
                        .build(&kit)
                        .grow(1.0)
                        .min_w(0.0),
                )
                .child(
                    TextField::new("Humedad (%)", &state.limits.rows[index].moisture)
                        .placeholder("%")
                        .on_edit(move |s: &mut AppState| &mut s.limits.rows[index].moisture)
                        .build(&kit)
                        .grow(1.0)
                        .min_w(0.0),
                )
        })
        .collect();

    let limits_result: Option<El> = state.limits.outcome.as_ref().map(|outcome| {
        let mut column = col()
            .key("limits-result")
            .w_full()
            .gap(2.0)
            .padding(10.0)
            .radius(kit.radius.sm)
            .fill(kit.c.bg_deep)
            .child(
                text(format!("Límite líquido: {:.1} %", outcome.liquid_limit))
                    .font(kit.ty.small)
                    .color(kit.c.text_base),
            )
            .child(
                text(format!(
                    "{} determinaciones · pendiente {:.2} · R² {:.3}",
                    outcome.determinations, outcome.slope, outcome.r_squared
                ))
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_muted),
            );
        if outcome.blows_outside_range {
            column = column.child(
                text("⚠ alguna determinación cae fuera del rango habitual de golpes")
                    .font(kit.ty.text_xsmall)
                    .color(kit.c.text_base),
            );
        }
        if let (Some(plastic), Some(index)) = (outcome.plastic_limit, outcome.plasticity_index) {
            column = column.child(
                text(format!(
                    "Límite plástico: {plastic:.1} % · índice de plasticidad: {index:.1}"
                ))
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_muted),
            );
        }
        column
    });

    let density_result: Option<El> = state.density.outcome.as_ref().map(|outcome| {
        let mut column = col()
            .key("density-result")
            .w_full()
            .gap(2.0)
            .padding(10.0)
            .radius(kit.radius.sm)
            .fill(kit.c.bg_deep)
            .child(
                text(format!(
                    "Densidad seca: {:.3} g/cm³ · compactación {:.1} %",
                    outcome.dry_density, outcome.relative_compaction
                ))
                .font(kit.ty.small)
                .color(kit.c.text_base),
            )
            .child(
                text(format!(
                    "Desviación de humedad: {:+.1} puntos",
                    outcome.moisture_deviation
                ))
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_muted),
            );
        for warning in &outcome.warnings {
            column = column.child(
                text(format!("⚠ {warning}"))
                    .font(kit.ty.text_xsmall)
                    .color(kit.c.text_base),
            );
        }
        column
    });

    let cbr_rows: Vec<El> = (0..state.cbr.rows.len())
        .map(|index| {
            row()
                .w_full()
                .gap(8.0)
                .child(
                    TextField::new("Penetración (mm)", &state.cbr.rows[index].penetration)
                        .placeholder("mm")
                        .on_edit(move |s: &mut AppState| &mut s.cbr.rows[index].penetration)
                        .build(&kit)
                        .grow(1.0)
                        .min_w(0.0),
                )
                .child(
                    TextField::new("Carga unitaria (MPa)", &state.cbr.rows[index].load)
                        .placeholder("MPa")
                        .on_edit(move |s: &mut AppState| &mut s.cbr.rows[index].load)
                        .build(&kit)
                        .grow(1.0)
                        .min_w(0.0),
                )
        })
        .collect();

    let cbr_result: Option<El> = state.cbr.outcome.as_ref().map(|outcome| {
        col()
            .key("cbr-result")
            .w_full()
            .gap(2.0)
            .padding(10.0)
            .radius(kit.radius.sm)
            .fill(kit.c.bg_deep)
            .child(
                text(format!("CBR: {:.1} %", outcome.percent))
                    .font(kit.ty.small)
                    .color(kit.c.text_base),
            )
            .child(
                text(format!(
                    "Gobierna la penetración de {:.2} mm · corrimiento de origen {:.2} mm",
                    outcome.governing_penetration, outcome.offset
                ))
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_muted),
            )
    });

    let soils_panel: El = col()
        .key("soils-panel")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .padding(12.0)
        .gap(8.0)
        .radius(kit.radius.md)
        .fill(kit.c.bg_base)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(
            text("Granulometría por tamices")
                .font(kit.ty.small)
                .color(kit.c.text_base),
        )
        .child(
            text("Masa seca total y lo retenido en cada tamiz. Las filas vacías se ignoran.")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_faint),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(field(
                    "Muestra",
                    "S-2026-0001",
                    &state.soils.sample,
                    |s: &mut AppState| &mut s.soils.sample,
                ))
                .child(field(
                    "Masa seca total (g)",
                    "1000",
                    &state.soils.total,
                    |s: &mut AppState| &mut s.soils.total,
                ))
                .child(action("add-sieve", "Añadir tamiz", |s: &mut AppState| {
                    s.soils_add_row()
                }))
                .child(action(
                    "calculate-granulometry",
                    "Calcular",
                    |s: &mut AppState| s.soils_calculate(),
                )),
        )
        .children(sieve_rows)
        .child(row().w_full().gap(8.0).center_y().child(action(
            "granulometry-pdf",
            "Informe PDF",
            |s: &mut AppState| s.generate_granulometry_report(),
        )))
        .children(state.soils.report_path.as_ref().map(|path| {
            text(format!("Informe generado en {path}"))
                .key("granulometry-report-path")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_muted)
                .padding(4.0)
        }))
        .children(soils_error)
        .children(soils_result)
        .child(el().w_full().h(1.0).fill(kit.c.layer_02))
        .child(
            text("Límites de Atterberg")
                .font(kit.ty.small)
                .color(kit.c.text_base),
        )
        .child(
            text("Humedad frente al logaritmo de los golpes; el límite es el valor a 25 golpes.")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_faint),
        )
        .children(limit_rows)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(action("add-limit-row", "Añadir determinación", |s: &mut AppState| {
                    s.limits_add_row()
                }))
                .child(action(
                    "calculate-limits",
                    "Calcular límites",
                    |s: &mut AppState| s.limits_calculate(),
                ))
                .child(action("atterberg-pdf", "Informe PDF", |s: &mut AppState| {
                    s.report_atterberg()
                })),
        )
        .children(state.limits.error.as_ref().map(|message| {
            text(format!("⚠ {message}"))
                .key("limits-error")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_base)
                .padding(4.0)
        }))
        .children(limits_result)
        .child(el().w_full().h(1.0).fill(kit.c.layer_02))
        .child(
            text("Densidad en sitio")
                .font(kit.ty.small)
                .color(kit.c.text_base),
        )
        .child(
            text("Densidad húmeda medida en campo y humedad; la densidad máxima de laboratorio es obligatoria.")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_faint),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(field(
                    "Densidad húmeda (g/cm³)",
                    "2.12",
                    &state.density.wet,
                    |s: &mut AppState| &mut s.density.wet,
                ))
                .child(field(
                    "Humedad (%)",
                    "10.0",
                    &state.density.moisture,
                    |s: &mut AppState| &mut s.density.moisture,
                ))
                .child(field(
                    "Densidad máxima (g/cm³)",
                    "1.95",
                    &state.density.max_dry,
                    |s: &mut AppState| &mut s.density.max_dry,
                ))
                .child(field(
                    "Humedad óptima (%)",
                    "11.0",
                    &state.density.optimum,
                    |s: &mut AppState| &mut s.density.optimum,
                )),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(action(
                    "calculate-density",
                    "Calcular densidad",
                    |s: &mut AppState| s.density_calculate(),
                ))
                .child(action("density-pdf", "Informe PDF", |s: &mut AppState| {
                    s.report_density()
                })),
        )
        .children(state.density.error.as_ref().map(|message| {
            text(format!("⚠ {message}"))
                .key("density-error")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_base)
                .padding(4.0)
        }))
        .children(density_result)
        .child(el().w_full().h(1.0).fill(kit.c.layer_02))
        .child(
            text("Compactación")
                .font(kit.ty.small)
                .color(kit.c.text_base),
        )
        .child(
            text("Cada punto sale de las masas del molde y de la muestra. El máximo se ajusta con todos los puntos.")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_faint),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(field(
                    "Molde + muestra (g)",
                    "5000",
                    &state.compaction.mold_plus,
                    |s: &mut AppState| &mut s.compaction.mold_plus,
                ))
                .child(field(
                    "Molde (g)",
                    "3000",
                    &state.compaction.mold,
                    |s: &mut AppState| &mut s.compaction.mold,
                ))
                .child(field(
                    "Volumen (cm³)",
                    "943",
                    &state.compaction.volume,
                    |s: &mut AppState| &mut s.compaction.volume,
                )),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(field(
                    "Muestra húmeda (g)",
                    "220",
                    &state.compaction.wet_sample,
                    |s: &mut AppState| &mut s.compaction.wet_sample,
                ))
                .child(field(
                    "Muestra seca (g)",
                    "200",
                    &state.compaction.dry_sample,
                    |s: &mut AppState| &mut s.compaction.dry_sample,
                ))
                .child(action(
                    "add-compaction-point",
                    "Añadir punto",
                    |s: &mut AppState| s.compaction_add_point(),
                )),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(action(
                    "calculate-compaction",
                    "Calcular máximo",
                    |s: &mut AppState| s.compaction_calculate(),
                ))
                .child(action("clear-compaction", "Limpiar puntos", |s: &mut AppState| {
                    s.compaction_clear()
                }))
                .child(action("compaction-pdf", "Informe PDF", |s: &mut AppState| {
                    s.report_compaction()
                })),
        )
        .children((!state.compaction.points.is_empty()).then(|| {
            col()
                .key("compaction-points")
                .w_full()
                .gap(1.0)
                .children(state.compaction.points.iter().map(|point| {
                    text(format!(
                        "{:.1} % · {:.3} g/cm³",
                        point.moisture, point.dry_density
                    ))
                    .font(kit.ty.text_xsmall)
                    .color(kit.c.text_faint)
                }))
        }))
        .children(state.compaction.error.as_ref().map(|message| {
            text(format!("⚠ {message}"))
                .key("compaction-error")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_base)
                .padding(4.0)
        }))
        .children(state.compaction.peak.as_ref().map(|peak| {
            let mut column = col()
                .key("compaction-result")
                .w_full()
                .gap(2.0)
                .padding(10.0)
                .radius(kit.radius.sm)
                .fill(kit.c.bg_deep)
                .child(
                    text(format!(
                        "Humedad óptima {:.1} % · densidad máxima {:.3} g/cm³",
                        peak.optimum_moisture, peak.max_dry_density
                    ))
                    .font(kit.ty.small)
                    .color(kit.c.text_base),
                )
                .child(
                    text(format!("{} puntos sostienen el ajuste", peak.points_used))
                        .font(kit.ty.text_xsmall)
                        .color(kit.c.text_muted),
                );
            if peak.below_max_measured {
                column = column.child(
                    text("⚠ el máximo ajustado queda por debajo del mayor valor medido: revísalo")
                        .font(kit.ty.text_xsmall)
                        .color(kit.c.text_base),
                );
            }
            column
        }))
        .child(el().w_full().h(1.0).fill(kit.c.layer_02))
        .child(text("CBR").font(kit.ty.small).color(kit.c.text_base))
        .child(
            text("Carga unitaria frente a penetración. La curva debe pasar de 5,08 mm.")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_faint),
        )
        .children(cbr_rows)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(action("add-cbr-row", "Añadir fila", |s: &mut AppState| {
                    s.cbr_add_row()
                }))
                .child(action("calculate-cbr", "Calcular CBR", |s: &mut AppState| {
                    s.cbr_calculate()
                }))
                .child(action("cbr-pdf", "Informe PDF", |s: &mut AppState| {
                    s.report_cbr()
                })),
        )
        .children(state.cbr.error.as_ref().map(|message| {
            text(format!("⚠ {message}"))
                .key("cbr-error")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_base)
                .padding(4.0)
        }))
        .children(cbr_result);

    // ------------------------------------------------ herramientas de la muestra
    let export_button = |key: &'static str, label: &'static str, format: ExportFormat| {
        el().key(key)
            .padding(8.0)
            .radius(kit.radius.sm)
            .fill(kit.c.layer_02)
            .focusable(true)
            .child(text(label).font(kit.ty.text_small).color(kit.c.text_base))
            .on_press(move |s: &mut AppState| s.export_tool(format))
    };

    let attachments_list: Option<El> = (!state.attachments.is_empty()).then(|| {
        col()
            .key("attachments")
            .w_full()
            .gap(1.0)
            .children(state.attachments.iter().map(|attachment| {
                text(attachment.clone())
                    .font(kit.ty.text_xsmall)
                    .color(kit.c.text_faint)
            }))
    });

    let tools_panel: El = col()
        .key("tools-panel")
        .w_full()
        .padding(10.0)
        .gap(6.0)
        .radius(kit.radius.md)
        .fill(kit.c.bg_base)
        .child(
            text("Herramientas de la muestra")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_muted),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(field(
                    "Muestra",
                    "H-2026-0001",
                    &state.tool_code,
                    |s: &mut AppState| &mut s.tool_code,
                ))
                .child(field(
                    "Archivo a adjuntar (ruta)",
                    "C:\\ruta\\foto.jpg",
                    &state.tool_path,
                    |s: &mut AppState| &mut s.tool_path,
                ))
                .child(action("attach-file", "Adjuntar", |s: &mut AppState| {
                    s.attach_tool()
                })),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(field(
                    "Motivo de anulación",
                    "muestra duplicada",
                    &state.tool_reason,
                    |s: &mut AppState| &mut s.tool_reason,
                ))
                .child(action("void-sample", "Anular", |s: &mut AppState| {
                    s.void_tool()
                }))
                .child(export_button(
                    "export-csv",
                    "Exportar CSV",
                    ExportFormat::Csv,
                ))
                .child(export_button(
                    "export-xlsx",
                    "Exportar XLSX",
                    ExportFormat::Xlsx,
                ))
                .child(action("tools-refresh", "Actualizar", |s: &mut AppState| {
                    s.load_tools()
                })),
        )
        .children(attachments_list);

    // ------------------------------------------------------------------ tabla
    let table = Table::new(
        |s: &AppState| &s.table,
        |s: &mut AppState| &mut s.table,
        |s: &AppState| s.rows.len(),
        |s: &AppState, index| std::borrow::Cow::Borrowed(s.rows[index].code.as_str()),
    )
    .name("Muestras")
    .column(
        TableColumn::new("code", "Código")
            .width(150.0)
            .min(110.0)
            .sortable(),
    )
    .column(
        TableColumn::new("material", "Material")
            .width(120.0)
            .min(90.0)
            .sortable(),
    )
    .column(
        TableColumn::new("client", "Cliente")
            .flex(1.0)
            .min(140.0)
            .sortable(),
    )
    .column(
        TableColumn::new("received", "Recepción")
            .width(120.0)
            .align(rune::Align::End)
            .sortable(),
    )
    .column(TableColumn::new("state", "Estado").width(110.0).sortable())
    .cell(move |s: &AppState, index, column| {
        let row = &s.rows[index];
        match column {
            "code" => Cell::text(&row.code).mono(),
            "material" => Cell::text(&row.material),
            "client" => Cell::text(&row.client),
            "received" => Cell::text(&row.received_on).mono(),
            _ => Cell::text(&row.state).muted(),
        }
    })
    .on_sort(|s: &mut AppState, column, direction, _| sort_rows(s, column, direction))
    .zebra(true)
    .build(state, &table_kit)
    .grow(1.0)
    .min_h(0.0);

    let table_card = col()
        .key("table-frame")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .radius(kit.radius.md)
        .clip_rounded(kit.radius.md)
        .fill(kit.c.bg_base)
        .child(table);

    // La sección de muestras combina la tabla con sus herramientas.
    let samples_body: El = col()
        .key("samples-body")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(8.0)
        .child(table_card)
        .child(tools_panel);

    // El área de trabajo muestra el formulario, el ensayo o las muestras.
    let body: El = if state.form_open {
        form_panel
    } else if state.nav == Nav::Concrete {
        test_panel
    } else if state.nav == Nav::Soils {
        soils_panel
    } else {
        samples_body
    };

    // ------------------------------------------------------------------ paleta
    let palette: Option<El> = state.palette_open.then(|| {
        col()
            .key("palette")
            .w_full()
            .padding(10.0)
            .gap(2.0)
            .radius(kit.radius.md)
            .fill(kit.c.layer_02)
            .child(
                text("Ir a una sección — Ctrl/⌘ + 1…6")
                    .font(kit.ty.text_small)
                    .color(kit.c.text_base),
            )
            .child(
                text("Esc para cerrar")
                    .font(kit.ty.text_xsmall)
                    .color(kit.c.text_faint),
            )
    });

    let empty_hint: Option<El> = (state.is_empty() && !state.form_open).then(|| {
        text("Todavía no hay muestras registradas. Usa «+ Nueva muestra» en Muestras.")
            .key("empty-hint")
            .font(kit.ty.text_xsmall)
            .color(kit.c.text_faint)
            .padding(8.0)
    });

    // ------------------------------------------------------------ barra inferior
    let status = row()
        .key("status")
        .w_full()
        .padding(8.0)
        .fill(kit.c.page)
        .child(
            text(state.status_line())
                .key("status-text")
                .font(kit.ty.text_xsmall)
                .color(kit.c.text_faint),
        );

    let main = col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .gap(10.0)
        .padding(12.0)
        .fill(kit.c.page)
        .child(header)
        .child(kpis)
        .children(palette)
        .children(flash)
        .children(empty_hint)
        .child(body)
        .child(status);

    row()
        .w_full()
        .h_full()
        .fill(kit.c.bg_deep)
        .child(sidebar)
        .child(main)
}
