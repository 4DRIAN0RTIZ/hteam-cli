use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::operations;

use super::app::{App, DetailField, HelpContext, ProjectDetailSection};
use super::widgets::{centered_rect, draw_frame};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    // Fondo/texto por defecto de toda la pantalla. Sin esto, cualquier celda
    // que ningún widget pinte explícitamente muestra el fondo nativo de la
    // terminal en vez del tema activo (ver `Style::patch`: un widget sin
    // `.bg()`/`.fg()` propios no toca lo que ya haya en esa celda).
    frame.render_widget(
        Block::default().style(
            Style::default()
                .bg(app.theme.background)
                .fg(app.theme.foreground),
        ),
        area,
    );
    let rows = if app.composing_search {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area)
    } else {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area)
    };

    draw_title_bar(frame, app, rows[0]);
    draw_board(frame, app, rows[1]);
    if app.composing_search {
        draw_search_bar(frame, app, rows[2]);
        draw_status_bar(frame, app, rows[3]);
    } else {
        draw_status_bar(frame, app, rows[2]);
    }

    if app.show_help {
        draw_help_popup(frame, app);
    } else if app.show_changelog {
        draw_changelog_popup(frame, app);
    } else if app.show_update_available {
        draw_update_available_popup(frame, app);
    } else if app.show_reminders {
        draw_reminders_popup(frame, app);
    } else if app.show_weekly_objectives {
        draw_weekly_objectives_popup(frame, app);
    } else if app.show_task_detail {
        draw_task_detail_popup(frame, app);
    } else if app.show_description {
        draw_description_popup(frame, app);
    } else if app.show_comments {
        draw_comments_popup(frame, app);
    } else if app.show_projects {
        draw_projects_popup(frame, app);
    } else if app.show_board_switch {
        draw_board_switch_popup(frame, app);
    } else if app.composing_card {
        draw_new_card_popup(frame, app);
    }
}

/// Título con "HTEAM #N" a la izquierda y, a la derecha, el estado de turno
/// del usuario (username · trabajando desde/salió a las HH:MM) seguido — si
/// hay `[working_hours]` configurado y estamos dentro del rango — del tiempo
/// restante de jornada en verde.
fn draw_title_bar(frame: &mut Frame, app: &App, area: Rect) {
    let title = Paragraph::new(format!(" HTEAM #{} ", app.board_number))
        .style(Style::default().add_modifier(Modifier::BOLD));

    let user_line = user_status_line(app);
    let remaining = app.working_hours_remaining();

    if user_line.is_none() && remaining.is_none() {
        frame.render_widget(title, area);
        return;
    }

    let user_width = user_line
        .as_ref()
        .map(|s| s.chars().count() as u16)
        .unwrap_or(0);
    let gap_width = if user_line.is_some() && remaining.is_some() {
        2
    } else {
        0
    };
    let remaining_width = remaining
        .as_ref()
        .map(|s| s.chars().count() as u16)
        .unwrap_or(0);

    let cols = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(user_width),
        Constraint::Length(gap_width),
        Constraint::Length(remaining_width),
    ])
    .split(area);

    frame.render_widget(title, cols[0]);

    if let Some(line) = user_line {
        frame.render_widget(Paragraph::new(line), cols[1]);
    }

    if let Some(remaining) = remaining {
        let working_hours = Paragraph::new(remaining)
            .style(
                Style::default()
                    .fg(app.theme.success)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Right);
        frame.render_widget(working_hours, cols[3]);
    }
}

/// Barra de estado inferior: mensaje de estado a la izquierda, versión (ej.
/// "v0.4.0") pegada a la orilla derecha.
fn draw_search_bar(frame: &mut Frame, app: &App, area: Rect) {
    let text = format!(" / {}_", app.search_input.as_str());
    let p = Paragraph::new(text).style(
        Style::default()
            .fg(app.theme.highlight)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(p, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status = app.footer_message().unwrap_or_else(|| {
        if let Some(summary) = app.card_filters_summary() {
            format!(" Filtro: {} · X limpiar · / editar · q salir", summary)
        } else {
            " / buscar · '?' ayuda · q salir".to_string()
        }
    });

    let version = format!("v{} ", env!("CARGO_PKG_VERSION"));
    let version_width = version.chars().count() as u16;

    let cols =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(version_width)]).split(area);

    frame.render_widget(Paragraph::new(status), cols[0]);
    frame.render_widget(
        Paragraph::new(version)
            .style(Style::default().fg(app.theme.muted))
            .alignment(Alignment::Right),
        cols[1],
    );
}

/// Título y contenido del popup de ayuda para `ctx` — ver `HelpContext`.
/// `Board` (sin ningún popup abierto) muestra el listado completo; el resto
/// muestra solo lo suyo, porque ya no entra en la línea de título de cada
/// popup sin truncarse.
fn help_content(ctx: HelpContext) -> (&'static str, &'static [&'static str]) {
    match ctx {
        HelpContext::Board => (
            " Ayuda ",
            &[
                "Board",
                "  j/k, ↓/↑        mover selección",
                "  h/l, ←/→        cambiar de lista",
                "  H / L           mover card a la lista anterior/siguiente",
                "  Enter           editar descripción de la card",
                "  d               detalle de la card (e edita el campo)",
                "  /               buscar / filtrar cards (texto, @user, #label)",
                "  X               limpiar filtro activo",
                "  n               crear card nueva en la lista activa",
                "  w               marcar/desmarcar la card activa como 'working on'",
                "  c               copiar el link de la card activa al portapapeles",
                "  v / V           ocultar lista activa / mostrar todas",
                "  r               refrescar",
                "  q, Esc          salir",
                "",
                "Popups (cada uno tiene su propia ayuda con '?')",
                "  B               cambiar de board",
                "  R               reminders",
                "  O               objetivos semanales",
                "  C               comentarios",
                "  P               proyectos",
                "",
                "Dentro de la descripción de una card",
                "  Enter           salto de línea",
                "  Ctrl+S          guardar",
                "  Esc             cancelar",
            ],
        ),
        HelpContext::Reminders => (
            " Ayuda · Reminders ",
            &[
                "Reminders",
                "  j/k, ↓/↑        seleccionar reminder",
                "  x               cancelar el reminder seleccionado",
                "  a               agregar un reminder para la card seleccionada",
                "  Esc, q          cerrar",
            ],
        ),
        HelpContext::WeeklyObjectives => (
            " Ayuda · Objetivos semanales ",
            &[
                "Objetivos semanales",
                "  j/k, ↓/↑        scroll",
                "  f               forzar refetch (ignora el cache)",
                "  Esc, q          cerrar",
            ],
        ),
        HelpContext::Comments => (
            " Ayuda · Comentarios ",
            &[
                "Comentarios",
                "  j/k, ↓/↑        scroll",
                "  a               escribir uno nuevo",
                "  v               completar el seguimiento agendado (si la card tiene uno)",
                "  x               cancelar el seguimiento agendado (si la card tiene uno)",
                "  Esc, q          cerrar",
                "",
                "Componiendo un comentario",
                "  Enter           salto de línea",
                "  Tab             alternar foco entre fecha y texto",
                "  Ctrl+F          marcar/desmarcar para seguimiento",
                "  Ctrl+S          enviar",
                "  Esc             cancelar",
                "",
                "Mientras escribís @algo",
                "  ↓/↑             navegar sugerencias de mención",
                "  Enter           aceptar la sugerencia resaltada",
            ],
        ),
        HelpContext::Projects => (
            " Ayuda · Proyectos ",
            &[
                "Proyectos guardados",
                "  a               agregar uno nuevo por ID",
                "  Tab             alternar foco (guardados / detalle)",
                "  j/k, ↓/↑        mover selección / milestones del detalle",
                "  Enter           elegir el resaltado",
                "  c               copiar el link del resaltado",
                "  L               listar proyectos en vivo",
                "  Esc, q          cerrar",
                "",
                "Detalle (foco con Tab)",
                "  h/l, ←/→        sección Milestones ↔ Tasks",
                "  j/k, ↓/↑        resaltar ítem de la sección",
                "  n               crear (milestone o task según sección)",
                "  e               editar el resaltado",
                "",
                "Formulario de milestone",
                "  Tab             alternar nombre / descripción (solo al crear)",
                "  Enter / Ctrl+S  guardar",
                "  Esc             cancelar",
                "  (edit)          nombre fijo — Hteam no permite renombrar",
                "",
                "Formulario de task",
                "  (create)        nombre, desc, start_date, dependence",
                "  (edit)          milestone, dependence, start_date",
                "  Tab             siguiente campo",
                "  Enter / Ctrl+S  guardar",
                "  Esc             cancelar",
                "",
                "Escribiendo un project id nuevo",
                "  Enter           confirmar",
                "  Esc             cancelar",
                "",
                "Listado en vivo ('L')",
                "  h/l, ←/→        cambiar estado (All/Planning/Execution/Finished/Cancelled)",
                "  j/k, ↓/↑        mover selección",
                "  Enter           elegir y cargar",
                "  c               copiar el link del resaltado",
                "  Esc, L          volver a guardados",
            ],
        ),
        HelpContext::TaskDetail => (
            " Ayuda · Detalle ",
            &[
                "Detalle de task",
                "  j/k             cambiar campo",
                "  e               editar el campo resaltado",
                "  Enter / Ctrl+S  guardar (en modo edición)",
                "  l               editar labels con picker multi-select",
                "  Esc             cancelar edición o cerrar",
                "  q               cerrar",
            ],
        ),
        HelpContext::BoardSwitch => (
            " Ayuda · Cambiar de board ",
            &[
                "Cambiar de board",
                "  j/k, ↓/↑        mover selección",
                "  Enter           seleccionar",
                "  r               recargar la lista",
                "  Esc, q          cerrar",
            ],
        ),
    }
}

fn draw_help_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 85, frame.area());
    let (title, lines) = help_content(app.help_context);
    let inner = draw_frame(
        frame,
        area,
        title,
        app.theme.border_help,
        app.theme.background,
        app.theme.foreground,
    );

    let p = Paragraph::new(lines.join("\n"))
        .wrap(Wrap { trim: true })
        .scroll((app.help_scroll.offset(), 0));
    frame.render_widget(p, inner);
}

fn draw_changelog_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 80, frame.area());
    let inner = draw_frame(
        frame,
        area,
        " CHANGELOG ",
        app.theme.border_help,
        app.theme.background,
        app.theme.foreground,
    );

    let body = app
        .startup_changelog
        .as_deref()
        .unwrap_or("(sin changelog)")
        .trim_end();
    let mut lines: Vec<Line> = body.lines().map(|l| Line::from(l.to_string())).collect();
    lines.push(Line::from(""));
    lines.push(muted_hint(app, "j/k scroll · c copiar link · Enter/Esc"));

    let p = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((app.changelog_scroll.offset(), 0));
    frame.render_widget(p, inner);
}

fn draw_update_available_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(55, 28, frame.area());
    let inner = draw_frame(
        frame,
        area,
        " Update Available ",
        app.theme.border_help,
        app.theme.background,
        app.theme.foreground,
    );

    let (current, latest) = match &app.update_available {
        Some(info) => (info.current_version.as_str(), info.latest_version.as_str()),
        None => ("?", "?"),
    };

    let mut lines = vec![
        Line::from("Hay una versión nueva de hteam."),
        Line::from(""),
        Line::from(format!("  actual:  {current}")),
        Line::from(format!("  última:  {latest}")),
        Line::from(""),
        Line::from("Ejecuta `hteam update` para instalarla."),
        Line::from(""),
        muted_hint(app, "c copiar releases · Enter/Esc"),
    ];
    let _ = &mut lines;

    let p = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    frame.render_widget(p, inner);
}

fn draw_new_card_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, frame.area());

    let list_name = app
        .new_card_list_id
        .and_then(|id| app.all_lists.iter().find(|l| l.id == id))
        .map(|l| l.name.as_str())
        .unwrap_or("?");

    let title = format!(" Nueva card · {} ", list_name);
    let inner = draw_frame(
        frame,
        area,
        title,
        app.theme.border_new_card,
        app.theme.background,
        app.theme.foreground,
    );

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let p = Paragraph::new(app.new_card_input.as_str()).wrap(Wrap { trim: true });
    frame.render_widget(p, chunks[0]);
    frame.render_widget(
        Paragraph::new(muted_hint(app, "Enter crear · Esc")),
        chunks[1],
    );
}

fn draw_reminders_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(68, 70, frame.area());
    let n = app.reminders.len();
    let title = if n == 0 {
        " Reminders ".to_string()
    } else {
        format!(" Reminders ({n}) ")
    };
    let inner = draw_frame(
        frame,
        area,
        title,
        app.theme.border_reminders,
        app.theme.background,
        app.theme.foreground,
    );

    let mut lines: Vec<Line> = Vec::new();
    lines.push(section_header(
        app,
        if n == 1 { "1 pendiente" } else { "Pendientes" },
    ));
    if n > 0 {
        lines.push(muted_hint(app, format!("{n} en total")));
    }
    lines.push(Line::from(""));

    if app.reminders.is_empty() {
        lines.push(empty_state_line(app, "Sin reminders pendientes."));
        lines.push(Line::from(""));
        lines.push(muted_hint(app, "Selecciona una card y pulsa a · ? · Esc"));
    } else {
        for (i, r) in app.reminders.iter().enumerate() {
            let selected = app.selected_reminder == Some(i);
            if i > 0 {
                lines.push(Line::from(Span::styled(
                    "  · · ·",
                    Style::default().fg(app.theme.muted),
                )));
            }
            let title = r
                .content_object
                .as_ref()
                .map(|c| c.title.as_str())
                .unwrap_or("(sin título)");
            let date_raw = r.reminder_date.as_deref().unwrap_or("");
            let date = if date_raw.is_empty() {
                "sin fecha".to_string()
            } else {
                // Show YYYY-MM-DD when the API includes a time suffix.
                date_raw.chars().take(10).collect()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if selected { "▶ " } else { "› " },
                    Style::default().fg(app.theme.border_reminders),
                ),
                Span::styled(
                    title.to_string(),
                    Style::default()
                        .fg(app.theme.highlight)
                        .add_modifier(Modifier::BOLD)
                        .bg(if selected {
                            app.theme.selection_bg
                        } else {
                            app.theme.background
                        }),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("fecha  ", Style::default().fg(app.theme.muted)),
                Span::styled(date, Style::default().fg(app.theme.foreground)),
                Span::styled(
                    format!("  · card {}", r.object_id),
                    Style::default().fg(app.theme.muted),
                ),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(muted_hint(
            app,
            "a nuevo · j/k seleccionar · x cancelar · ? · Esc",
        ));
    }

    let p = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((app.reminders_scroll.offset(), 0));
    frame.render_widget(p, inner);
}

fn draw_weekly_objectives_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 70, frame.area());
    let inner = draw_frame(
        frame,
        area,
        " Objetivos semanales ",
        app.theme.border_objectives,
        app.theme.background,
        app.theme.foreground,
    );

    let mut lines: Vec<Line> = if let Some(error) = &app.weekly_objectives_error {
        vec![
            Line::from("No se pudieron cargar los objetivos semanales."),
            Line::from(""),
            Line::from(error.clone()),
        ]
    } else if let Some(set) = &app.weekly_objectives {
        let source = if app.weekly_objectives_from_cache {
            "cache"
        } else {
            "red"
        };
        let mut lines = vec![Line::from(format!("🎯 {}", set.title)), Line::from("")];

        for objective in &set.objectives {
            lines.push(Line::from(format!("## {}", objective.name)));
            for task in &objective.tasks {
                let mark = if task.done { "x" } else { " " };
                lines.push(Line::from(format!("   [{}] {}", mark, task.description)));
            }
            lines.push(Line::from(""));
        }

        if let Some(synced_at) = app.weekly_objectives_synced_at {
            lines.push(Line::from(format!(
                "{}, {}",
                source,
                relative_time(synced_at)
            )));
        }
        lines
    } else {
        vec![Line::from(Span::styled(
            "Cargando objetivos semanales…",
            Style::default()
                .fg(app.theme.muted)
                .add_modifier(Modifier::ITALIC),
        ))]
    };
    lines.push(Line::from(""));
    lines.push(muted_hint(app, "f refetch · j/k scroll · ? · Esc"));

    let p = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: true })
        .scroll((app.weekly_objectives_scroll.offset(), 0));
    frame.render_widget(p, inner);
}

fn relative_time(synced_at: chrono::DateTime<chrono::Utc>) -> String {
    let elapsed = chrono::Utc::now().signed_duration_since(synced_at);

    if elapsed.num_hours() > 0 {
        format!(
            "sincronizado hace {}h {}m",
            elapsed.num_hours(),
            elapsed.num_minutes() % 60
        )
    } else if elapsed.num_minutes() > 0 {
        format!("sincronizado hace {}m", elapsed.num_minutes())
    } else {
        format!("sincronizado hace {}s", elapsed.num_seconds().max(0))
    }
}

fn draw_description_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 70, frame.area());

    let card_name = app.current_card().map(|c| c.name.as_str()).unwrap_or("?");
    let title = format!(" Descripción · {} ", card_name);
    let inner = draw_frame(
        frame,
        area,
        title,
        app.theme.border_description,
        app.theme.background,
        app.theme.foreground,
    );

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let text_width = chunks[0].width.saturating_sub(2).max(1);
    let content_rows = wrapped_line_count(app.description_input.as_str(), text_width);
    let scroll_y = content_rows.saturating_sub(chunks[0].height);
    let p = Paragraph::new(app.description_input.as_str())
        .wrap(Wrap { trim: true })
        .scroll((scroll_y, 0));
    frame.render_widget(p, chunks[0]);
    frame.render_widget(
        Paragraph::new(muted_hint(app, "Enter salto · Ctrl+S guardar · Esc")),
        chunks[1],
    );
}

fn draw_task_detail_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(74, 82, frame.area());
    let title = if app.picking_labels {
        let f = app.label_picker_filter.as_str();
        if f.is_empty() {
            " Labels ".to_string()
        } else {
            format!(" Labels · filtro: {f}_ ")
        }
    } else if app.picking_dependence {
        let f = app.dependence_picker_filter.as_str();
        if f.is_empty() {
            " Depende de ".to_string()
        } else {
            format!(" Depende de · filtro: {f}_ ")
        }
    } else if app.editing_detail_field {
        let field = DetailField::ALL[app.task_detail_field_idx.min(DetailField::ALL.len() - 1)];
        format!(" ✎ {} ", field.label())
    } else {
        " Detalle ".to_string()
    };
    let inner = draw_frame(
        frame,
        area,
        title,
        app.theme.border_description,
        app.theme.background,
        app.theme.foreground,
    );

    let lines: Vec<Line> = if app.picking_labels {
        label_picker_lines(app)
    } else if app.picking_dependence {
        dependence_picker_lines(app)
    } else if let Some(err) = &app.task_detail_error {
        vec![
            Line::from(Span::styled(
                "No se pudo cargar el detalle",
                Style::default()
                    .fg(app.theme.danger)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                err.clone(),
                Style::default().fg(app.theme.muted),
            )),
        ]
    } else if let Some(detail) = &app.task_detail {
        task_detail_lines(app, detail)
    } else {
        vec![Line::from(Span::styled(
            "Cargando detalle…",
            Style::default()
                .fg(app.theme.muted)
                .add_modifier(Modifier::ITALIC),
        ))]
    };

    let p = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((app.task_detail_scroll.offset(), 0));
    frame.render_widget(p, inner);
}

fn label_picker_lines(app: &App) -> Vec<Line<'static>> {
    use super::events::filtered_label_picker_indices;
    let mut lines = vec![section_header(app, "Seleccionar labels"), Line::from("")];
    if app.label_picker_loading {
        lines.push(muted_hint(app, "Cargando catálogo de labels…"));
        return lines;
    }
    let indices = filtered_label_picker_indices(app);
    if indices.is_empty() {
        lines.push(empty_state_line(
            app,
            if app.label_picker_items.is_empty() {
                "No hay labels disponibles en el catálogo."
            } else {
                "Ninguna label coincide con el filtro."
            },
        ));
        lines.push(Line::from(""));
        lines.push(muted_hint(app, "escribe para filtrar · Esc cancela"));
        return lines;
    }
    let selected_position = indices
        .iter()
        .position(|index| *index == app.label_picker_idx)
        .unwrap_or(0);
    let window = 14usize;
    let start = selected_position.saturating_sub(window / 2);
    let end = (start + window).min(indices.len());
    let start = end.saturating_sub(window);
    if start > 0 {
        lines.push(muted_hint(app, format!("▲ {} más arriba", start)));
    }
    for &index in &indices[start..end] {
        let label = &app.label_picker_items[index];
        let selected = index == app.label_picker_idx;
        let marked = app.label_picker_selected.contains(&label.id);
        let style = if selected {
            selected_row_style(app)
        } else {
            Style::default()
        };
        let marker = if selected { "› " } else { "  " };
        let check = if marked { "[x] " } else { "[ ] " };
        let color = parse_hex_color(&label.color).unwrap_or(app.theme.foreground);
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(check, style),
            Span::styled(
                format!("{}  (id {})", label.name, label.id),
                if selected {
                    style
                } else {
                    Style::default().fg(color)
                },
            ),
        ]));
    }
    if end < indices.len() {
        lines.push(muted_hint(
            app,
            format!("▼ {} más abajo", indices.len() - end),
        ));
    }
    lines.push(Line::from(""));
    lines.push(muted_hint(
        app,
        "j/k · Espacio marca/desmarca · Enter guarda · escribe para filtrar · Esc",
    ));
    lines
}

fn dependence_picker_lines(app: &App) -> Vec<Line<'static>> {
    use super::events::filtered_dependence_picker_indices;
    let mut lines = vec![
        section_header(app, "Elegir task dependencia"),
        Line::from(""),
    ];
    let idxs = filtered_dependence_picker_indices(app);
    if idxs.is_empty() {
        lines.push(empty_state_line(
            app,
            if app.dependence_picker_items.is_empty() {
                "No hay otras tasks cargadas en el board."
            } else {
                "Ninguna task coincide con el filtro."
            },
        ));
        lines.push(Line::from(""));
        lines.push(muted_hint(app, "escribe para filtrar · Esc"));
        return lines;
    }
    // Keep selection near middle of a window of ~12 rows.
    let sel_pos = idxs
        .iter()
        .position(|i| *i == app.dependence_picker_idx)
        .unwrap_or(0);
    let window = 14usize;
    let start = sel_pos.saturating_sub(window / 2);
    let end = (start + window).min(idxs.len());
    let start = end.saturating_sub(window);
    if start > 0 {
        lines.push(muted_hint(app, format!("▲ {} más arriba", start)));
    }
    for &item_i in &idxs[start..end] {
        let (id, num, name) = &app.dependence_picker_items[item_i];
        let sel = item_i == app.dependence_picker_idx;
        let style = if sel {
            selected_row_style(app)
        } else {
            Style::default().fg(app.theme.foreground)
        };
        let marker = if sel { "› " } else { "  " };
        let num_s = num
            .map(|n| format!("#{n}"))
            .unwrap_or_else(|| format!("id {id}"));
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(format!("{num_s:<8} "), style),
            Span::styled(name.clone(), style),
            Span::styled(format!("  ({id})"), Style::default().fg(app.theme.muted)),
        ]));
    }
    if end < idxs.len() {
        lines.push(muted_hint(app, format!("▼ {} más abajo", idxs.len() - end)));
    }
    lines.push(Line::from(""));
    lines.push(muted_hint(
        app,
        "j/k · Enter confirma · x quita · escribe para filtrar · Esc",
    ));
    lines
}

fn task_detail_lines<'a>(app: &'a App, detail: &'a crate::models::CardDetail) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = Vec::new();
    let selected = DetailField::ALL[app.task_detail_field_idx.min(DetailField::ALL.len() - 1)];

    // Header
    let mut title_spans = Vec::new();
    if let Some(n) = detail.number {
        title_spans.push(Span::styled(
            format!("#{n} "),
            Style::default()
                .fg(app.theme.highlight)
                .add_modifier(Modifier::BOLD),
        ));
    }
    title_spans.push(Span::styled(
        detail.name.clone(),
        Style::default()
            .fg(app.theme.foreground)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::from(title_spans));
    lines.push(Line::from(Span::styled(
        format!("id {} · task detail", detail.id),
        Style::default().fg(app.theme.muted),
    )));

    if let Some(list) = &detail.list {
        lines.push(Line::from(vec![
            Span::styled("lista  ", Style::default().fg(app.theme.muted)),
            Span::styled(
                list.name.clone(),
                Style::default().fg(app.theme.border_active),
            ),
        ]));
    }
    if detail.closed == Some(true) {
        lines.push(Line::from(Span::styled(
            "cerrada",
            Style::default()
                .fg(app.theme.danger)
                .add_modifier(Modifier::BOLD),
        )));
    }

    lines.push(Line::from(""));
    lines.push(section_header(app, "Información"));
    for f in [
        DetailField::Name,
        DetailField::Description,
        DetailField::Responsible,
        DetailField::Priority,
        DetailField::Dependence,
    ] {
        lines.push(detail_field_line(app, detail, f, selected));
    }

    lines.push(Line::from(""));
    lines.push(section_header(app, "Fechas"));
    for f in [DetailField::StartDate, DetailField::DueDate] {
        lines.push(detail_field_line(app, detail, f, selected));
    }

    lines.push(Line::from(""));
    lines.push(section_header(app, "Tiempo"));
    for f in [DetailField::TimeEstimated, DetailField::TimeSpent] {
        lines.push(detail_field_line(app, detail, f, selected));
    }

    lines.push(Line::from(""));
    lines.push(section_header(app, "Labels"));
    lines.push(detail_labels_line(app, detail));

    lines.push(Line::from(""));
    if app.editing_detail_field {
        let field = DetailField::ALL[app.task_detail_field_idx.min(DetailField::ALL.len() - 1)];
        lines.push(muted_hint(
            app,
            format!("{} · Enter/Ctrl+S guardar · Esc cancelar", field.hint()),
        ));
    } else {
        lines.push(muted_hint(
            app,
            "j/k campo · e editar · l labels · Enter en Depende de = saltar · ? · Esc",
        ));
    }

    lines
}

/// Shared popup chrome: section divider used by detail, comments, etc.
fn section_header<'a>(app: &App, title: &'a str) -> Line<'a> {
    Line::from(Span::styled(
        format!("── {title} ──"),
        Style::default()
            .fg(app.theme.border_active)
            .add_modifier(Modifier::BOLD),
    ))
}

fn muted_hint(app: &App, text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into(),
        Style::default().fg(app.theme.muted),
    ))
}

fn empty_state_line<'a>(app: &App, text: &'a str) -> Line<'a> {
    Line::from(Span::styled(
        text,
        Style::default()
            .fg(app.theme.muted)
            .add_modifier(Modifier::ITALIC),
    ))
}

fn selected_row_style(app: &App) -> Style {
    Style::default()
        .bg(app.theme.selection_bg)
        .fg(app.theme.selection_fg)
        .add_modifier(Modifier::BOLD)
}

fn detail_field_line<'a>(
    app: &'a App,
    detail: &'a crate::models::CardDetail,
    field: DetailField,
    selected: DetailField,
) -> Line<'a> {
    let is_sel = field == selected;
    let is_edit = is_sel && app.editing_detail_field;
    let label = format!("{:<12}", field.label());
    let raw = if is_edit {
        format!("{}▌", app.detail_edit_input.as_str())
    } else {
        detail_field_value(app, detail, field)
    };

    let empty = raw == "-" && !is_edit;
    let row_style = if is_sel {
        selected_row_style(app)
    } else {
        Style::default()
    };
    let label_style = if is_sel {
        row_style
    } else {
        Style::default().fg(app.theme.muted)
    };
    let value_style = if is_sel {
        row_style
    } else if empty {
        Style::default().fg(app.theme.muted)
    } else if field == DetailField::Responsible {
        Style::default().fg(app.theme.highlight)
    } else {
        Style::default().fg(app.theme.foreground)
    };

    let marker = if is_sel { "› " } else { "  " };
    Line::from(vec![
        Span::styled(marker, row_style),
        Span::styled(label, label_style),
        Span::styled(" ", row_style),
        Span::styled(raw, value_style),
    ])
}

fn detail_labels_line<'a>(app: &'a App, detail: &'a crate::models::CardDetail) -> Line<'a> {
    if detail.labels.is_empty() {
        return Line::from(Span::styled(
            "  (sin labels)",
            Style::default().fg(app.theme.muted),
        ));
    }
    let mut spans = vec![Span::raw("  ")];
    for (i, lab) in detail.labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let fg = parse_hex_color(&lab.color).unwrap_or(app.theme.highlight);
        spans.push(Span::styled(
            format!("[{}]", lab.name),
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

fn parse_hex_color(s: &str) -> Option<ratatui::style::Color> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(ratatui::style::Color::Rgb(r, g, b))
}

fn detail_field_value(app: &App, detail: &crate::models::CardDetail, field: DetailField) -> String {
    match field {
        DetailField::Name => detail.name.clone(),
        DetailField::Description => detail
            .description
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "-".into()),
        DetailField::Responsible => operations::cards::responsible_display(
            detail
                .responsible
                .as_ref()
                .unwrap_or(&serde_json::Value::Null),
        )
        .unwrap_or_else(|| "-".into()),
        DetailField::Priority => detail
            .priority
            .as_ref()
            .map(operations::cards::priority_label)
            .filter(|s| !s.is_empty() && s != "null" && s != "-")
            .unwrap_or_else(|| "-".into()),
        DetailField::Dependence => app
            .task_detail_dependence_label
            .clone()
            .or_else(|| operations::cards::dependence_display(&detail.dependence))
            .unwrap_or_else(|| "-".into()),
        DetailField::StartDate => detail
            .start_date
            .as_deref()
            .map(|s| s.chars().take(10).collect::<String>())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".into()),
        DetailField::DueDate => detail
            .due_date
            .as_deref()
            .map(|s| s.chars().take(10).collect::<String>())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".into()),
        DetailField::TimeEstimated => detail
            .time_estimated
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "-".into()),
        DetailField::TimeSpent => detail
            .time_spend
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "-".into()),
    }
}

fn draw_comments_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(72, 75, frame.area());

    let title = if app.composing_comment && app.comment_reply_to.is_some() {
        " Responder comentario "
    } else if app.composing_comment {
        " Comentario nuevo "
    } else {
        " Comentarios "
    };
    let inner = draw_frame(
        frame,
        area,
        title,
        app.theme.border_comments,
        app.theme.background,
        app.theme.foreground,
    );

    if !app.composing_comment {
        draw_comment_list(frame, app, inner);
        return;
    }

    // composing_comment is true below — list-only path returned above.
    let text_width = inner.width.saturating_sub(2).max(1);
    let content_rows = wrapped_line_count(app.comment_input.as_str(), text_width);
    let suggestion_rows = app.mention_suggestions.len().min(5) as u16;

    // Grow with content (+1 spare row for the cursor line, +2 for the
    // input block's own top/bottom borders, +3 for the fixed-height date
    // field above it), but never shrink the comment list above below 3
    // rows — that's what keeps this from breaking the popup layout on a
    // long comment.
    let wanted = content_rows + suggestion_rows + 1 + 2 + DATE_FIELD_HEIGHT;
    let max_input_h = inner.height.saturating_sub(3).max(4 + DATE_FIELD_HEIGHT);
    let input_h = wanted.clamp(4 + DATE_FIELD_HEIGHT, max_input_h);

    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(input_h)]).split(inner);
    draw_comment_list(frame, app, chunks[0]);
    draw_comment_input(frame, app, chunks[1], text_width);
}

const DATE_FIELD_HEIGHT: u16 = 3;

fn draw_comment_list(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    if !app.composing_comment {
        let n = app.comments.len();
        lines.push(section_header(
            app,
            if n == 1 {
                "1 comentario"
            } else {
                // lifetime: section_header needs 'a str — use static for empty/count via leak-free owned
                "Comentarios"
            },
        ));
        if n > 0 {
            lines.push(muted_hint(app, format!("{n} en total")));
        }
        lines.push(Line::from(""));
    }

    if app.comments.is_empty() {
        lines.push(empty_state_line(app, "Sin comentarios todavía."));
        if !app.composing_comment {
            lines.push(Line::from(""));
            lines.push(muted_hint(app, "a escribir · j/k · ? · Esc"));
        }
    } else {
        for (i, c) in app.comments.iter().enumerate() {
            let selected = app.comments_selected == Some(i);
            if i > 0 {
                lines.push(Line::from(Span::styled(
                    "  · · ·",
                    Style::default().fg(app.theme.muted),
                )));
            }
            lines.push(Line::from(vec![
                Span::styled(
                    if selected { "▶ " } else { "› " },
                    Style::default().fg(app.theme.border_comments),
                ),
                Span::styled(
                    c.user_name.as_str(),
                    Style::default()
                        .fg(app.theme.highlight)
                        .add_modifier(Modifier::BOLD)
                        .bg(if selected {
                            app.theme.selection_bg
                        } else {
                            app.theme.background
                        }),
                ),
                Span::styled(
                    format!("  {}", c.submit_date),
                    Style::default().fg(app.theme.muted),
                ),
            ]));
            // Indent body under author
            for body_line in c.comment.lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(body_line, Style::default().fg(app.theme.foreground)),
                ]));
            }
            // Follow-up hangs off the specific comment (same id), like the web.
            if let Some(follow_up) = &app.current_follow_up {
                if follow_up.comment_id == c.id {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("⏱ seguimiento {}", follow_up.date),
                            Style::default()
                                .fg(app.theme.warning)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "  · v completar · x cancelar",
                            Style::default().fg(app.theme.muted),
                        ),
                    ]));
                }
            }
            lines.push(Line::from(""));
        }
        if !app.composing_comment {
            lines.push(muted_hint(
                app,
                "a nuevo · j/k seleccionar · r responder · ? · Esc",
            ));
        }
    }

    let p = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((app.comments_scroll.offset(), 0));
    frame.render_widget(p, area);
}

fn draw_comment_input(frame: &mut Frame, app: &App, area: Rect, text_width: u16) {
    let chunks =
        Layout::vertical([Constraint::Length(DATE_FIELD_HEIGHT), Constraint::Min(1)]).split(area);
    draw_comment_date_field(frame, app, chunks[0]);
    draw_comment_text_field(frame, app, chunks[1], text_width);
}

fn draw_comment_date_field(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.comment_date_focused;
    let border_color = if focused {
        app.theme.selection_bg
    } else {
        app.theme.muted
    };
    let block = Block::default()
        .title(if focused { " › Fecha " } else { " Fecha " })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let empty = app.comment_date_input.as_str().is_empty();
    let text = if empty {
        "ahora (hora local)".to_string()
    } else {
        format!("{}▌", app.comment_date_input.as_str())
    };
    let style = if focused {
        selected_row_style(app)
    } else if empty {
        Style::default().fg(app.theme.muted)
    } else {
        Style::default().fg(app.theme.foreground)
    };
    frame.render_widget(Paragraph::new(text).style(style), inner);
}

fn draw_comment_text_field(frame: &mut Frame, app: &App, area: Rect, text_width: u16) {
    let focused = !app.comment_date_focused;
    let border_color = if focused {
        app.theme.selection_bg
    } else {
        app.theme.muted
    };
    let title = if focused {
        if app.comment_follow {
            " › Comentario · seguimiento ON "
        } else {
            " › Comentario "
        }
    } else if app.comment_follow {
        " Comentario · seguimiento ON "
    } else {
        " Comentario "
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content_rows = wrapped_line_count(app.comment_input.as_str(), text_width);
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        format!("{}▌", app.comment_input.as_str()),
        if focused {
            Style::default().fg(app.theme.foreground)
        } else {
            Style::default().fg(app.theme.muted)
        },
    ))];

    if !app.mention_suggestions.is_empty() {
        lines.push(Line::from(""));
        lines.push(section_header(app, "Menciones"));
        for (i, user) in app.mention_suggestions.iter().take(5).enumerate() {
            let sel = i == app.mention_selected;
            let style = if sel {
                selected_row_style(app)
            } else {
                Style::default().fg(app.theme.foreground)
            };
            let marker = if sel { "› " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(user.text.as_str(), style),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(muted_hint(
        app,
        "@ mención · Tab fecha · f seguimiento · Ctrl+S enviar · Esc",
    ));

    // Keep the cursor line visible when the box is height-capped.
    let scroll_y = content_rows.saturating_sub(inner.height);
    let p = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));
    frame.render_widget(p, inner);
}

/// Rough greedy word-wrap line counter — doesn't need to match ratatui's
/// `Wrap { trim: true }` byte-for-byte, just closely enough to size the
/// input box sensibly as the comment grows.
fn wrapped_line_count(text: &str, width: u16) -> u16 {
    let width = width.max(1) as usize;
    let mut total: u16 = 0;

    for line in text.split('\n') {
        if line.is_empty() {
            total += 1;
            continue;
        }

        let mut rows: u16 = 1;
        let mut cur_len: usize = 0;
        for word in line.split_whitespace() {
            let wlen = word.chars().count();
            if cur_len == 0 {
                cur_len = wlen;
            } else if cur_len + 1 + wlen <= width {
                cur_len += 1 + wlen;
            } else {
                rows += 1;
                cur_len = wlen;
            }
            while cur_len > width {
                rows += 1;
                cur_len -= width;
            }
        }
        total += rows;
    }

    total.max(1)
}

fn draw_projects_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(80, 75, frame.area());

    let title = if app.composing_project_task {
        if app.project_task_form_edit_id.is_some() {
            " Editar task de proyecto ".to_string()
        } else {
            " Nueva task de proyecto ".to_string()
        }
    } else if app.composing_milestone {
        if app.milestone_form_edit_id.is_some() {
            " Editar milestone ".to_string()
        } else {
            " Nuevo milestone ".to_string()
        }
    } else if app.composing_project {
        " Nuevo project id ".to_string()
    } else if app.show_live_projects {
        format!(
            " Proyectos en vivo ({}) ",
            operations::projects::PROJECT_STATUSES[app.live_projects_status_idx]
        )
    } else {
        " Proyectos ".to_string()
    };
    let inner = draw_frame(
        frame,
        area,
        title,
        app.theme.border_projects,
        app.theme.background,
        app.theme.foreground,
    );

    let form_rows = if app.composing_project_task {
        if app.project_task_form_edit_id.is_some() {
            3
        } else {
            4
        }
    } else if app.composing_milestone {
        2
    } else if app.composing_project {
        1
    } else {
        0
    };

    let rows = if form_rows > 0 {
        let mut constraints = vec![Constraint::Min(0)];
        for _ in 0..form_rows {
            constraints.push(Constraint::Length(3));
        }
        Layout::vertical(constraints).split(inner)
    } else {
        Layout::vertical([Constraint::Min(0)]).split(inner)
    };

    if app.show_live_projects {
        draw_live_projects_list(frame, app, rows[0]);
    } else {
        let cols = Layout::horizontal([Constraint::Length(12), Constraint::Min(0)]).split(rows[0]);
        draw_known_projects_list(frame, app, cols[0]);
        draw_project_detail(frame, app, cols[1]);
    }

    if app.composing_project_task {
        draw_project_task_form(frame, app, &rows[1..]);
    } else if app.composing_milestone {
        draw_milestone_form(frame, app, rows[1], rows[2]);
    } else if app.composing_project {
        draw_project_input(frame, app, rows[1]);
    }
}

fn draw_known_projects_list(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Guardados ")
        .borders(Borders::ALL)
        .border_style(if !app.projects_detail_focused {
            Style::default().fg(app.theme.border_active)
        } else {
            Style::default()
        });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = if app.known_projects.is_empty() {
        "(vacío)".to_string()
    } else {
        app.known_projects
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let marker = if i == app.selected_project_idx {
                    "▶"
                } else {
                    " "
                };
                let active = if Some(*id) == app.active_project {
                    " •"
                } else {
                    ""
                };
                format!("{} #{}{}", marker, id, active)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
}

fn draw_live_projects_list(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Proyectos ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border_active));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = if app.live_projects.is_empty() {
        "(sin proyectos)".to_string()
    } else {
        app.live_projects
            .iter()
            .enumerate()
            .map(|(i, project)| {
                let marker = if i == app.selected_live_project_idx {
                    "▶"
                } else {
                    " "
                };
                let active = if Some(project.id) == app.active_project {
                    " •"
                } else {
                    ""
                };
                format!("{} #{} {}{}", marker, project.id, project.name, active)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
}

fn draw_project_detail(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Milestones / Tasks ")
        .borders(Borders::ALL)
        .border_style(if app.projects_detail_focused {
            Style::default().fg(app.theme.border_active)
        } else {
            Style::default()
        });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(id) = app.active_project else {
        frame.render_widget(
            Paragraph::new("Sin proyecto seleccionado. 'a' agrega uno nuevo, o elegí uno guardado con j/k + Enter.")
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    };

    let composing = app.composing_milestone || app.composing_project_task;
    // Sticky footer so create/edit shortcuts stay visible while the body scrolls.
    let body_area = if app.projects_detail_focused && !composing {
        let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
        let section = match app.projects_detail_section {
            ProjectDetailSection::Milestones => "milestones",
            ProjectDetailSection::Tasks => "tasks",
        };
        let footer = Line::from(vec![
            Span::styled(" h/l ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{section}  "), Style::default().fg(app.theme.muted)),
            Span::styled("n ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("crear  ", Style::default().fg(app.theme.muted)),
            Span::styled("e ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("editar  ", Style::default().fg(app.theme.muted)),
            Span::styled("j/k ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("sel  ", Style::default().fg(app.theme.muted)),
            Span::styled("? ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("ayuda", Style::default().fg(app.theme.muted)),
        ]);
        frame.render_widget(Paragraph::new(footer), rows[1]);
        rows[0]
    } else if !app.projects_detail_focused && !composing {
        let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
        let footer = Line::from(vec![
            Span::styled(" Tab ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                "foco aquí para milestones/tasks",
                Style::default().fg(app.theme.muted),
            ),
        ]);
        frame.render_widget(Paragraph::new(footer), rows[1]);
        rows[0]
    } else {
        inner
    };

    let mut lines = vec![format!("Proyecto #{}", id), String::new()];
    let ms_active = app.projects_detail_focused
        && app.projects_detail_section == ProjectDetailSection::Milestones;
    let task_active =
        app.projects_detail_focused && app.projects_detail_section == ProjectDetailSection::Tasks;

    if app.project_milestones.is_empty() {
        lines.push(if ms_active {
            "Milestones: (vacío)".to_string()
        } else {
            "Milestones:".to_string()
        });
        if ms_active {
            lines.push("  n crear milestone".to_string());
        }
    } else {
        let head = if ms_active {
            "Milestones: ●".to_string()
        } else {
            "Milestones:".to_string()
        };
        lines.push(head);
        for (i, m) in app.project_milestones.iter().enumerate() {
            let marker = if ms_active && i == app.selected_milestone_idx {
                "▶"
            } else {
                " "
            };
            let id_suffix = m.id().map(|id| format!(" #{id}")).unwrap_or_default();
            lines.push(format!(
                "{} {} {}{}",
                marker,
                progress_bar(m.progress),
                m.name,
                id_suffix
            ));
        }
    }

    lines.push(String::new());

    if app.project_tasks.is_empty() {
        lines.push(if task_active {
            "Tasks: ● (vacío)".to_string()
        } else {
            "Tasks:".to_string()
        });
        if task_active {
            lines.push("  n crear task (usa milestone resaltado)".to_string());
        }
    } else {
        let head = if task_active {
            "Tasks: ●".to_string()
        } else {
            "Tasks:".to_string()
        };
        lines.push(head);
        for (i, t) in app.project_tasks.iter().enumerate() {
            let marker = if task_active && i == app.selected_task_idx {
                "▶"
            } else {
                " "
            };
            let mark = if t.closed { "✅" } else { "🔲" };
            let ms = t
                .milestone
                .as_ref()
                .map(|m| format!(" · m{}", m.id))
                .unwrap_or_default();
            lines.push(format!(
                "{} {} #{} {}{}",
                marker, mark, t.number, t.name, ms
            ));
        }
    }

    let p = Paragraph::new(lines.join("\n"))
        .wrap(Wrap { trim: true })
        .scroll((app.projects_scroll.offset(), 0));
    frame.render_widget(p, body_area);
}

fn draw_project_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Project id (solo números) ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(app.project_input.as_str()), inner);
}

fn draw_project_task_form(frame: &mut Frame, app: &App, areas: &[Rect]) {
    let edit = app.project_task_form_edit_id.is_some();
    let fields: &[(&str, &str)] = if edit {
        &[
            (" Milestone id ", app.project_task_milestone_input.as_str()),
            (
                " Dependence id (vacío = sin cambio) ",
                app.project_task_dependence_input.as_str(),
            ),
            (
                " Start date YYYY-MM-DD ",
                app.project_task_start_date_input.as_str(),
            ),
        ]
    } else {
        &[
            (" Nombre ", app.project_task_name_input.as_str()),
            (
                " Descripción (opcional) ",
                app.project_task_desc_input.as_str(),
            ),
            (
                " Start date YYYY-MM-DD (opcional) ",
                app.project_task_start_date_input.as_str(),
            ),
            (
                " Dependence id (opcional) ",
                app.project_task_dependence_input.as_str(),
            ),
        ]
    };
    for (i, area) in areas.iter().enumerate() {
        let Some((title_base, value)) = fields.get(i).copied() else {
            break;
        };
        let focused = app.project_task_form_field == i;
        let title = if focused {
            format!("{title_base}● ")
        } else {
            title_base.to_string()
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if focused {
                Style::default().fg(app.theme.border_active)
            } else {
                Style::default()
            });
        let inner = block.inner(*area);
        frame.render_widget(block, *area);
        frame.render_widget(Paragraph::new(value), inner);
    }
}

fn draw_milestone_form(frame: &mut Frame, app: &App, name_area: Rect, desc_area: Rect) {
    let name_locked = app.milestone_form_edit_id.is_some();
    let name_title = if name_locked {
        " Nombre (fijo — Hteam no renombra) "
    } else if app.milestone_desc_focused {
        " Nombre "
    } else {
        " Nombre ● "
    };
    let name_block = Block::default()
        .title(name_title)
        .borders(Borders::ALL)
        .border_style(if name_locked {
            Style::default().fg(app.theme.muted)
        } else if !app.milestone_desc_focused {
            Style::default().fg(app.theme.border_active)
        } else {
            Style::default()
        });
    let name_inner = name_block.inner(name_area);
    frame.render_widget(name_block, name_area);
    let name_style = if name_locked {
        Style::default().fg(app.theme.muted)
    } else {
        Style::default()
    };
    frame.render_widget(
        Paragraph::new(app.milestone_name_input.as_str()).style(name_style),
        name_inner,
    );

    let desc_hint = if name_locked {
        " Descripción "
    } else {
        " Descripción (opcional) "
    };
    let desc_title = if app.milestone_desc_focused || name_locked {
        format!("{desc_hint}● ")
    } else {
        desc_hint.to_string()
    };
    let desc_block = Block::default()
        .title(desc_title)
        .borders(Borders::ALL)
        .border_style(if app.milestone_desc_focused || name_locked {
            Style::default().fg(app.theme.border_active)
        } else {
            Style::default()
        });
    let desc_inner = desc_block.inner(desc_area);
    frame.render_widget(desc_block, desc_area);
    frame.render_widget(
        Paragraph::new(app.milestone_desc_input.as_str()),
        desc_inner,
    );
}

/// Milestone progress comes as 0.0–1.0 from the API — a simple 10-segment bar.
fn progress_bar(v: f64) -> String {
    let pct = (v.clamp(0.0, 1.0) * 100.0).round() as usize;
    let filled = pct / 10;
    format!("{}{} {}%", "█".repeat(filled), "░".repeat(10 - filled), pct)
}

fn draw_board_switch_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(78, 65, frame.area());
    let inner = draw_frame(
        frame,
        area,
        " Cambiar board ",
        app.theme.border_board_switch,
        app.theme.background,
        app.theme.foreground,
    );

    if app.available_boards.is_empty() {
        let msg = if app.is_loading_boards() {
            "Cargando boards..."
        } else {
            "No se pudieron cargar los boards. Presiona 'r' para reintentar."
        };
        frame.render_widget(Paragraph::new(msg), inner);
        return;
    }

    let lines: Vec<Line> = app
        .available_boards
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let marker = if i == app.selected_board_idx {
                "▶"
            } else {
                " "
            };
            let active = if b.id == app.board_number {
                " •"
            } else {
                "  "
            };
            let name = trunc_str(&b.name, 28);
            let service = trunc_str(&b.service, 32);
            let tasks = format!("{}/{}", b.total_tasks_closed, b.total_tasks);

            let text = format!(
                "{} {:>5}  {:<28}  {:<32}  {:>8}{}",
                marker,
                format!("#{}", b.id),
                name,
                service,
                tasks,
                active
            );

            let style = if i == app.selected_board_idx {
                Style::default()
                    .fg(app.theme.selection_fg)
                    .bg(app.theme.selection_bg)
            } else if b.id == app.board_number {
                Style::default().fg(app.theme.highlight)
            } else {
                Style::default()
            };

            Line::from(text).style(style)
        })
        .collect();

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let scroll_y = if app.selected_board_idx + 1 > chunks[0].height as usize {
        (app.selected_board_idx + 1 - chunks[0].height as usize) as u16
    } else {
        0
    };
    let p = Paragraph::new(Text::from(lines)).scroll((scroll_y, 0));
    frame.render_widget(p, chunks[0]);
    frame.render_widget(
        Paragraph::new(muted_hint(app, "j/k · Enter seleccionar · ? · Esc")),
        chunks[1],
    );
}

fn trunc_str(s: &str, max: usize) -> String {
    let chars: String = s.chars().take(max).collect();
    format!("{:<width$}", chars, width = max)
}

/// Reloj de Nerd Font (nf-fa-clock_o, U+F017) usado como indicador de
/// urgencia de seguimiento en cada card. A diferencia de un emoji (🕐), es un
/// glifo monocromo que sí respeta el fg de `Style` — la mayoría de terminales
/// pintan los emoji con su propia fuente a todo color, ignorándolo. Requiere
/// una Nerd Font en la terminal; sin una, se ve como un glifo faltante.
const FOLLOW_UP_ICON: char = '\u{f017}';

fn draw_board(frame: &mut Frame, app: &App, area: Rect) {
    let visible = app.visible_lists();
    if visible.is_empty() {
        let msg = if app.all_lists.is_empty() {
            "Sin listas para mostrar. Presiona 'r' para reintentar."
        } else {
            "Todas las listas están ocultas. Presiona 'V' para mostrarlas."
        };
        frame.render_widget(Paragraph::new(msg), area);
        return;
    }

    let n = visible.len() as u32;
    let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Ratio(1, n)).collect();
    let columns = Layout::horizontal(constraints).split(area);

    for (i, list) in visible.into_iter().enumerate() {
        let is_active_list = i == app.selected_list;
        let cards = app.view_cards(list.id);
        let selected_idx = if is_active_list && !cards.is_empty() {
            Some(app.current_card_index().min(cards.len() - 1))
        } else {
            None
        };

        // Measure with the inner width of a bordered column.
        let probe = Block::default().borders(Borders::ALL);
        let inner_w = probe.inner(columns[i]).width;
        let inner_h = probe.inner(columns[i]).height;
        let title_width = inner_w.saturating_sub(2);
        let heights: Vec<u16> = cards
            .iter()
            .map(|c| wrapped_line_count(&c.name, title_width) + 1 + 2)
            .collect();

        let scroll_start =
            column_scroll_start(inner_h, &heights, selected_idx.filter(|_| is_active_list));

        // How many cards fit from scroll_start for the title cue.
        let mut used = 0u16;
        let mut last_drawn = scroll_start.saturating_sub(1);
        for (ci, h) in heights.iter().enumerate().skip(scroll_start) {
            if used.saturating_add(*h) > inner_h {
                break;
            }
            used = used.saturating_add(*h);
            last_drawn = ci;
        }
        let hidden_above = scroll_start;
        let hidden_below = cards.len().saturating_sub(last_drawn.saturating_add(1));
        let title = match (hidden_above > 0, hidden_below > 0) {
            (true, true) => format!(" {} ▲{} ▼{} ", list.name, hidden_above, hidden_below),
            (true, false) => format!(" {} ▲{} ", list.name, hidden_above),
            (false, true) => format!(" {} ▼{} ", list.name, hidden_below),
            _ => format!(" {} ", list.name),
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if is_active_list {
                Style::default().fg(app.theme.border_active)
            } else {
                Style::default()
            });
        let inner = block.inner(columns[i]);
        frame.render_widget(block, columns[i]);

        let mut y = inner.y;
        for (ci, card) in cards.iter().enumerate().skip(scroll_start) {
            let card_h = heights[ci];
            if y + card_h > inner.y + inner.height {
                break;
            }
            let rect = Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: card_h,
            };

            // List name / Follow-up label are already shown elsewhere — skip noise.
            let labels = card
                .labels
                .iter()
                .filter(|l| !l.name.eq_ignore_ascii_case(&list.name) && !l.is_follow_up())
                .map(|l| format!("#{}", l.name))
                .collect::<Vec<_>>()
                .join(" ");

            let is_working = app.is_working_on(card.id);

            let mut second_line = vec![Span::raw(labels)];
            if let Some(status) = &card.time_status {
                let color = app.theme.time_status_color(status);
                second_line.push(Span::raw(" "));
                second_line.push(Span::styled(
                    FOLLOW_UP_ICON.to_string(),
                    Style::default().fg(color),
                ));
            }
            let text = Text::from(vec![
                Line::from(card.name.as_str()),
                Line::from(second_line),
            ]);

            let is_selected = selected_idx == Some(ci);
            let body_style = if is_selected {
                Style::default()
                    .fg(app.theme.selection_fg)
                    .bg(app.theme.selection_bg)
            } else {
                Style::default()
            };
            let border_style = if is_working {
                Style::default().fg(app.theme.highlight)
            } else {
                Style::default()
            };

            let p = Paragraph::new(text)
                .style(body_style)
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(border_style),
                );
            frame.render_widget(p, rect);
            y += card_h;
        }
    }
}

/// First card index to draw so `selected` stays inside `viewport_h`.
fn column_scroll_start(viewport_h: u16, heights: &[u16], selected: Option<usize>) -> usize {
    if heights.is_empty() {
        return 0;
    }
    let Some(sel) = selected else {
        return 0;
    };
    let sel = sel.min(heights.len() - 1);

    // If everything from 0..=sel fits, no scroll.
    let mut top_to_sel = 0u16;
    for h in heights.iter().take(sel + 1) {
        top_to_sel = top_to_sel.saturating_add(*h);
    }
    if top_to_sel <= viewport_h {
        return 0;
    }

    // Scroll so the selection sits on the bottom edge of the viewport
    // (vim-style: grow upward from sel until we fill the column).
    let mut used = heights[sel];
    let mut start = sel;
    while start > 0 {
        let prev = heights[start - 1];
        if used.saturating_add(prev) > viewport_h {
            break;
        }
        used = used.saturating_add(prev);
        start -= 1;
    }
    start
}

/// Texto del estado de turno mostrado a la derecha del header, p.ej.
/// "adrian.ortiz · trabajando desde 07:55 " o "adrian.ortiz · salió a las 17:01 ".
fn user_status_line(app: &App) -> Option<String> {
    let shift = app.user_shift.as_ref()?;
    let username = &shift.user.username;
    let line = match (&shift.check_in, &shift.check_out) {
        (Some(check_in), None) => {
            format!("{} · trabajando desde {} ", username, format_time(check_in))
        }
        (Some(_), Some(check_out)) => {
            format!("{} · salió a las {} ", username, format_time(check_out))
        }
        _ => format!("{} ", username),
    };
    Some(line)
}

fn format_time(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

#[cfg(test)]
mod board_scroll_tests {
    use super::column_scroll_start;

    #[test]
    fn no_selection_starts_at_zero() {
        assert_eq!(column_scroll_start(20, &[4, 4, 4, 4, 4], None), 0);
    }

    #[test]
    fn selection_in_view_stays_at_zero() {
        assert_eq!(column_scroll_start(20, &[4, 4, 4, 4], Some(2)), 0);
    }

    #[test]
    fn selection_past_viewport_scrolls() {
        // 6 cards x 4h = 24 > 10 viewport; select last → start near end
        let heights = vec![4u16; 6];
        let start = column_scroll_start(10, &heights, Some(5));
        assert!(start > 0, "must scroll down, got {start}");
        // from start..=5 must fit in 10
        let used: u16 = heights[start..=5].iter().sum();
        assert!(used <= 10, "window too tall: {used}");
        assert!(heights[start..=5].contains(&4));
        assert_eq!(start, 4); // sel=5: grow up → start=4 (8h), start=3 would be 12h > 10
    }
}
