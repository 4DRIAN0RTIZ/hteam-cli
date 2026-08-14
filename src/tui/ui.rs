use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::app::App;

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

    let status = app
        .status
        .clone()
        .unwrap_or_else(|| " '?' ayuda · q salir".to_string());
    frame.render_widget(Paragraph::new(status), rows[2]);

    if app.show_help {
        draw_help_popup(frame);
    } else if app.show_reminders {
        draw_reminders_popup(frame, app);
    } else if app.show_description {
        draw_description_popup(frame, app);
    } else if app.show_comments {
        draw_comments_popup(frame, app);
    } else if app.show_projects {
        draw_projects_popup(frame, app);
    } else if app.composing_card {
        draw_new_card_popup(frame, app);
    }
}

/// Título con "HTEAM #N" a la izquierda y, si hay `[working_hours]`
/// configurado y estamos dentro del rango, el tiempo restante a la derecha.
fn draw_title_bar(frame: &mut Frame, app: &App, area: Rect) {
    let title = Paragraph::new(format!(" HTEAM #{} ", app.board_number))
        .style(Style::default().add_modifier(Modifier::BOLD));

    let Some(remaining) = app.working_hours_remaining() else {
        frame.render_widget(title, area);
        return;
    };

    let cols = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(remaining.chars().count() as u16),
    ])
    .split(area);

    frame.render_widget(title, cols[0]);
    let working_hours = Paragraph::new(remaining)
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Right);
    frame.render_widget(working_hours, cols[1]);
}

fn draw_help_popup(frame: &mut Frame) {
    let area = centered_rect(70, 85, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Ayuda — Esc/q/? cerrar ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));
    let inner = block.inner(area);
    frame.render_widget(block, area);

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
        "  R               reminders — 'a' agrega uno para la card seleccionada",
        "  C               comentarios — 'a' escribe uno nuevo (@ para mencionar)",
        "  P               proyectos/milestones — 'a' nuevo project id, j/k+Enter elige uno guardado",
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

    let p = Paragraph::new(text).wrap(Wrap { trim: true });
    frame.render_widget(p, inner);
}

fn draw_new_card_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, frame.area());
    frame.render_widget(Clear, area);

    let list_name = app
        .new_card_list_id
        .and_then(|id| app.all_lists.iter().find(|l| l.id == id))
        .map(|l| l.name.as_str())
        .unwrap_or("?");

    let block = Block::default()
        .title(format!(" Nueva card en {} — Enter crear · Esc cancelar ", list_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let p = Paragraph::new(app.new_card_input.as_str()).wrap(Wrap { trim: true });
    frame.render_widget(p, inner);
}

fn draw_reminders_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 60, frame.area());
    frame.render_widget(Clear, area);

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

    let block = Block::default()
        .title(" Reminders — 'a' agregar de la card seleccionada · Esc cerrar ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let p = Paragraph::new(lines.join("\n")).block(block).wrap(Wrap { trim: true });
    frame.render_widget(p, area);
}

fn draw_description_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    let card_name = app.current_card().map(|c| c.name.as_str()).unwrap_or("?");
    let block = Block::default()
        .title(format!(
            " Descripción de \"{}\" — Enter salto de línea · Ctrl+S guardar · Esc cancelar ",
            card_name
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text_width = inner.width.saturating_sub(2).max(1);
    let content_rows = wrapped_line_count(&app.description_input, text_width);
    let scroll_y = content_rows.saturating_sub(inner.height);
    let p = Paragraph::new(app.description_input.as_str())
        .wrap(Wrap { trim: true })
        .scroll((scroll_y, 0));
    frame.render_widget(p, inner);
}

fn draw_comments_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    let title = if app.composing_comment {
        " Nuevo comentario — Enter salto de línea · Ctrl+S enviar · Esc cancelar "
    } else {
        " Comentarios — 'a' agregar · Esc cerrar "
    };
    let outer = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if app.composing_comment {
        // Text width inside the input block, minus its own left/right borders.
        let text_width = inner.width.saturating_sub(2).max(1);
        let content_rows = wrapped_line_count(&app.comment_input, text_width);
        let suggestion_rows = app.mention_suggestions.len().min(5) as u16;

        // Grow with content (+1 spare row for the cursor line, +2 for the
        // input block's own top/bottom borders), but never shrink the
        // comment list above below 3 rows — that's what keeps this from
        // breaking the popup layout on a long comment.
        let wanted = content_rows + suggestion_rows + 1 + 2;
        let max_input_h = inner.height.saturating_sub(3).max(4);
        let input_h = wanted.clamp(4, max_input_h);

        let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(input_h)]).split(inner);
        draw_comment_list(frame, app, chunks[0]);
        draw_comment_input(frame, app, chunks[1], text_width);
    } else {
        draw_comment_list(frame, app, inner);
    }
}

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
    let p = Paragraph::new(text).wrap(Wrap { trim: true });
    frame.render_widget(p, area);
}

fn draw_comment_input(frame: &mut Frame, app: &App, area: Rect, text_width: u16) {
    let block = Block::default()
        .title(" Comentario (@ para mencionar) ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content_rows = wrapped_line_count(&app.comment_input, text_width);
    let mut text = app.comment_input.clone();
    if !app.mention_suggestions.is_empty() {
        for (i, user) in app.mention_suggestions.iter().take(5).enumerate() {
            let marker = if i == app.mention_selected { "▶" } else { " " };
            text.push_str(&format!("\n{} {}", marker, user.text));
        }
    }

    // Once the box has hit its cap, scroll so the lines you just typed stay
    // in view instead of silently clipping off the bottom.
    let scroll_y = content_rows.saturating_sub(inner.height);
    let p = Paragraph::new(text).wrap(Wrap { trim: true }).scroll((scroll_y, 0));
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
    frame.render_widget(Clear, area);

    let title = if app.composing_project {
        " Nuevo project id — dígitos + Enter · Esc cancelar "
    } else {
        " Proyectos — 'a' agregar · j/k + Enter elegir · Esc cerrar "
    };
    let outer = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

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
    let block = Block::default().title(" Guardados ").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = if app.known_projects.is_empty() {
        "(vacío)".to_string()
    } else {
        app.known_projects
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let marker = if i == app.selected_project_idx { "▶" } else { " " };
                let active = if Some(*id) == app.active_project { " •" } else { "" };
                format!("{} #{}{}", marker, id, active)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
}

fn draw_project_detail(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Milestones / Tasks ").borders(Borders::ALL);
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

    let p = Paragraph::new(lines.join("\n")).wrap(Wrap { trim: true });
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

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
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
                .block(Block::default().borders(Borders::ALL).border_style(border_style));
            frame.render_widget(p, rect);
        }
    }
}
