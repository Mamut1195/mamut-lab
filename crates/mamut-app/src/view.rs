//! La vista: una función del estado, sin efectos.
//!
//! No consulta la base de datos, no calcula y no decide conformidad: presenta lo que el
//! estado ya contiene. Todo lo que se ve aquí se puede probar sin abrir una ventana.

use rune::{Cx, El, Key, KeyEvent, Tx, col, el, row, text};
use rune_kit::{Cell, Kit, SortDir, Table, TableColumn};

use crate::state::{AppState, Nav, SampleRow};

/// Atajos de teclado del armazón.
///
/// Devuelve `true` si la tecla quedó atendida, que es lo que Rune espera del manejador.
pub fn handle_key(state: &mut AppState, key: &KeyEvent) -> bool {
    if key.is_command('k') {
        state.toggle_palette();
        return true;
    }

    if matches!(&key.key, Key::Escape) {
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
    let header = row()
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
        )
        .child(el().grow(1.0).h(1.0))
        .child(
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
        .child(kpi("Muestras registradas", state.total_rows().to_string()))
        .child(kpi("Normas verificadas", state.norms_verified.to_string()))
        .child(kpi("Normas pendientes", state.norms_pending.to_string()))
        .child(kpi("Almacenamiento", "Local".to_string()));

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

    let empty_hint: Option<El> = state.is_empty().then(|| {
        text("Todavía no hay muestras registradas. El alta de muestras llega en la fase 2.")
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
        .children(empty_hint)
        .child(table_card)
        .child(status);

    row()
        .w_full()
        .h_full()
        .fill(kit.c.bg_deep)
        .child(sidebar)
        .child(main)
}
