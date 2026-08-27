use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::app::App;
use super::widgets::{centered_rect, draw_frame};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    draw_title_bar(frame, app, rows[0]);

    draw_board(frame, app, rows[1]);

    draw_status_bar(frame, app, rows[2]);

    if app.show_help {
        draw_help_popup(frame, app);
    } else if app.show_reminders {
        draw_reminders_popup(frame, app);
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
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Right);
        frame.render_widget(working_hours, cols[3]);
    }
}

/// Barra de estado inferior: mensaje de estado a la izquierda, versión (ej.
/// "v0.4.0") pegada a la orilla derecha.
fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status = app
        .status
        .clone()
        .unwrap_or_else(|| " '?' ayuda · q salir".to_string());

    let version = format!("v{} ", env!("CARGO_PKG_VERSION"));
    let version_width = version.chars().count() as u16;

    let cols =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(version_width)]).split(area);

    frame.render_widget(Paragraph::new(status), cols[0]);
    frame.render_widget(
        Paragraph::new(version)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Right),
        cols[1],
    );
}

fn draw_help_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 85, frame.area());
    let inner = draw_frame(
        frame,
        area,
        " Ayuda — j/k scroll · Esc/q/? cerrar ",
        Color::White,
    );

    let text = [
        "Board",
        "  j/k, ↓/↑        mover selección",
        "  h/l, ←/→        cambiar de lista",
        "  H / L           mover card a la lista anterior/siguiente",
        "  Enter           editar descripción de la card",
        "  n               crear card nueva en la lista activa",
        "  w               marcar/desmarcar la card activa como 'working on'",
        "  v / V           ocultar lista activa / mostrar todas",
        "  r               refrescar",
        "  q, Esc          salir",
        "",
        "Popups",
        "  B               cambiar de board — lista en vivo · j/k + Enter seleccionar",
        "  j/k, ↓/↑        scroll del contenido (reminders, comentarios, esta ayuda)",
        "  R               reminders — 'a' agrega uno para la card seleccionada",
        "  C               comentarios — 'a' escribe uno nuevo (@ para mencionar, Tab fecha, Ctrl+F seguimiento)",
        "  P               proyectos — 'a' nuevo · Tab foco al detalle · j/k mover/scroll · Enter elegir",
        "",
        "Dentro de un popup de texto (nueva card / project id)",
        "  Enter           confirmar",
        "  Esc             cancelar",
        "",
        "Dentro de la descripción o un comentario",
        "  Enter           salto de línea",
        "  Ctrl+S          guardar / enviar",
        "  Esc             cancelar",
        "",
        "Dentro del popup de comentarios, mientras escribís @algo",
        "  ↓/↑             navegar sugerencias de mención",
        "  Enter           aceptar la sugerencia resaltada",
    ]
    .join("\n");

    let p = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .scroll((app.help_scroll.offset(), 0));
    frame.render_widget(p, inner);
}

fn draw_new_card_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, frame.area());

    let list_name = app
        .new_card_list_id
        .and_then(|id| app.all_lists.iter().find(|l| l.id == id))
        .map(|l| l.name.as_str())
        .unwrap_or("?");

    let title = format!(" Nueva card en {} — Enter crear · Esc cancelar ", list_name);
    let inner = draw_frame(frame, area, title, Color::Cyan);

    let p = Paragraph::new(app.new_card_input.as_str()).wrap(Wrap { trim: true });
    frame.render_widget(p, inner);
}

fn draw_reminders_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 60, frame.area());
    let inner = draw_frame(
        frame,
        area,
        " Reminders — 'a' agregar · j/k scroll · Esc cerrar ",
        Color::Magenta,
    );

    let lines: Vec<String> = if app.reminders.is_empty() {
        vec!["Sin reminders pendientes.".to_string()]
    } else {
        app.reminders
            .iter()
            .map(|r| {
                let title = r
                    .content_object
                    .as_ref()
                    .map(|c| c.title.as_str())
                    .unwrap_or("(sin título)");
                let date = r.reminder_date.as_deref().unwrap_or("sin fecha");
                format!("• {} — {}", title, date)
            })
            .collect()
    };

    let p = Paragraph::new(lines.join("\n"))
        .wrap(Wrap { trim: true })
        .scroll((app.reminders_scroll.offset(), 0));
    frame.render_widget(p, inner);
}

fn draw_description_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 70, frame.area());

    let card_name = app.current_card().map(|c| c.name.as_str()).unwrap_or("?");
    let title = format!(
        " Descripción de \"{}\" — Enter salto de línea · Ctrl+S guardar · Esc cancelar ",
        card_name
    );
    let inner = draw_frame(frame, area, title, Color::Blue);

    let text_width = inner.width.saturating_sub(2).max(1);
    let content_rows = wrapped_line_count(app.description_input.as_str(), text_width);
    let scroll_y = content_rows.saturating_sub(inner.height);
    let p = Paragraph::new(app.description_input.as_str())
        .wrap(Wrap { trim: true })
        .scroll((scroll_y, 0));
    frame.render_widget(p, inner);
}

fn draw_comments_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 70, frame.area());

    let title = if app.composing_comment {
        " Nuevo comentario — Enter salto de línea · Tab fecha/texto · Ctrl+F seguimiento · Ctrl+S enviar · Esc cancelar "
    } else {
        " Comentarios — 'a' agregar · j/k scroll · Esc cerrar "
    };
    let inner = draw_frame(frame, area, title, Color::Blue);

    if app.composing_comment {
        // Text width inside the input block, minus its own left/right borders.
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

        let chunks =
            Layout::vertical([Constraint::Min(0), Constraint::Length(input_h)]).split(inner);
        draw_comment_list(frame, app, chunks[0]);
        draw_comment_input(frame, app, chunks[1], text_width);
    } else {
        draw_comment_list(frame, app, inner);
    }
}

const DATE_FIELD_HEIGHT: u16 = 3;

fn draw_comment_list(frame: &mut Frame, app: &App, area: Rect) {
    let text = if app.comments.is_empty() {
        "Sin comentarios todavía.".to_string()
    } else {
        app.comments
            .iter()
            .map(|c| format!("{} ({}):\n{}\n", c.user_name, c.submit_date, c.comment))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let p = Paragraph::new(text)
        .wrap(Wrap { trim: true })
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
    let border_color = if app.comment_date_focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .title(" Fecha (YYYY-MM-DD HH:MM) — Tab enfoca ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = if app.comment_date_input.as_str().is_empty() {
        "ahora (hora local)".to_string()
    } else {
        app.comment_date_input.as_str().to_string()
    };
    let style = if app.comment_date_input.as_str().is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };
    frame.render_widget(Paragraph::new(text).style(style), inner);
}

fn draw_comment_text_field(frame: &mut Frame, app: &App, area: Rect, text_width: u16) {
    let border_color = if app.comment_date_focused {
        Color::DarkGray
    } else {
        Color::Yellow
    };
    let title = if app.comment_follow {
        " Comentario (@ para mencionar) · 🔔 seguimiento ON "
    } else {
        " Comentario (@ para mencionar) "
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content_rows = wrapped_line_count(app.comment_input.as_str(), text_width);
    let mut text = app.comment_input.as_str().to_string();
    if !app.mention_suggestions.is_empty() {
        for (i, user) in app.mention_suggestions.iter().take(5).enumerate() {
            let marker = if i == app.mention_selected {
                "▶"
            } else {
                " "
            };
            text.push_str(&format!("\n{} {}", marker, user.text));
        }
    }

    // Once the box has hit its cap, scroll so the lines you just typed stay
    // in view instead of silently clipping off the bottom.
    let scroll_y = content_rows.saturating_sub(inner.height);
    let p = Paragraph::new(text)
        .wrap(Wrap { trim: true })
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

    let title = if app.composing_project {
        " Nuevo project id — dígitos + Enter · Esc cancelar "
    } else {
        " Proyectos — Tab foco · j/k mover/scroll · 'a' agregar · Esc cerrar "
    };
    let inner = draw_frame(frame, area, title, Color::Green);

    let rows = if app.composing_project {
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(inner)
    } else {
        Layout::vertical([Constraint::Min(0)]).split(inner)
    };

    let cols = Layout::horizontal([Constraint::Length(12), Constraint::Min(0)]).split(rows[0]);
    draw_known_projects_list(frame, app, cols[0]);
    draw_project_detail(frame, app, cols[1]);

    if app.composing_project {
        draw_project_input(frame, app, rows[1]);
    }
}

fn draw_known_projects_list(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Guardados ")
        .borders(Borders::ALL)
        .border_style(if !app.projects_detail_focused {
            Style::default().fg(Color::Cyan)
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

fn draw_project_detail(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Milestones / Tasks ")
        .borders(Borders::ALL)
        .border_style(if app.projects_detail_focused {
            Style::default().fg(Color::Cyan)
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

    let mut lines = vec![format!("Proyecto #{}", id), String::new()];

    if app.project_milestones.is_empty() {
        lines.push("Sin milestones.".to_string());
    } else {
        lines.push("Milestones:".to_string());
        for m in &app.project_milestones {
            lines.push(format!("  {} {}", progress_bar(m.progress), m.name));
        }
    }

    lines.push(String::new());

    if app.project_tasks.is_empty() {
        lines.push("Sin tasks.".to_string());
    } else {
        lines.push("Tasks:".to_string());
        for t in &app.project_tasks {
            let mark = if t.closed { "✅" } else { "🔲" };
            lines.push(format!("  {} #{} {}", mark, t.number, t.name));
        }
    }

    let p = Paragraph::new(lines.join("\n"))
        .wrap(Wrap { trim: true })
        .scroll((app.projects_scroll.offset(), 0));
    frame.render_widget(p, inner);
}

fn draw_project_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Project id (solo números) ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(app.project_input.as_str()), inner);
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
        " Cambiar board — j/k · Enter seleccionar · r recargar · Esc cerrar ",
        Color::Yellow,
    );

    if app.available_boards.is_empty() {
        let msg = if app
            .status
            .as_deref()
            .map(|s| s.contains("Cargando"))
            .unwrap_or(false)
        {
            "Cargando boards..."
        } else {
            "No se pudieron cargar los boards. Presiona 'r' para reintentar."
        };
        frame.render_widget(Paragraph::new(msg), inner);
        return;
    }

    // Scroll automático para mantener el ítem seleccionado en pantalla
    let scroll_y = if app.selected_board_idx + 1 > inner.height as usize {
        (app.selected_board_idx + 1 - inner.height as usize) as u16
    } else {
        0
    };

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
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if b.id == app.board_number {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            Line::from(text).style(style)
        })
        .collect();

    let p = Paragraph::new(Text::from(lines)).scroll((scroll_y, 0));
    frame.render_widget(p, inner);
}

/// Trunca `s` a `max` caracteres y rellena con espacios si es más corto.
fn trunc_str(s: &str, max: usize) -> String {
    let chars: String = s.chars().take(max).collect();
    format!("{:<width$}", chars, width = max)
}

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
        let block = Block::default()
            .title(format!(" {} ", list.name))
            .borders(Borders::ALL)
            .border_style(if is_active_list {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });
        let inner = block.inner(columns[i]);
        frame.render_widget(block, columns[i]);

        let cards = app.cards_by_list.get(&list.id);
        let selected_idx = if is_active_list {
            Some(app.current_card_index())
        } else {
            None
        };

        let card_h: u16 = 3;
        for (ci, card) in cards.into_iter().flatten().enumerate() {
            let y = inner.y + (ci as u16) * card_h;
            if y + card_h > inner.y + inner.height {
                break;
            }
            let rect = Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: card_h,
            };

            let labels = card
                .labels
                .iter()
                .map(|l| format!("#{}", l.name))
                .collect::<Vec<_>>()
                .join(" ");

            let is_working = app.is_working_on(card.id);
            let text = format!("{}\n{}", card.name, labels);

            let is_selected = selected_idx == Some(ci);
            let body_style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            let border_style = if is_working {
                Style::default().fg(Color::Yellow)
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
        }
    }
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
