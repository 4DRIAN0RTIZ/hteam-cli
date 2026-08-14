use anyhow::Result;

use crate::client::HteamClient;

use super::app::App;

pub fn open_help(app: &mut App) {
    app.show_help = true;
    app.help_scroll = 0;
}

pub fn close_help(app: &mut App) {
    app.show_help = false;
}

pub async fn refresh_all(client: &HteamClient, app: &mut App) {
    app.set_status("Cargando...");

    refresh_user_shift(client, app).await;

    match client.get_lists(Some(app.board_number)).await {
        Ok(lists) => {
            app.all_lists = lists;
            app.cards_by_list.clear();

            let mut error_msg = None;
            for list in app.all_lists.clone() {
                match client.get_cards(list.id, Some(app.board_number)).await {
                    Ok((cards, _)) => {
                        app.cards_by_list.insert(list.id, cards);
                    }
                    Err(e) => {
                        error_msg = Some(format!("Error cargando '{}': {}", list.name, e));
                    }
                }
            }

            refresh_working_on(client, app).await;

            let visible_len = app.visible_lists().len();
            app.selected_list = if visible_len > 0 {
                app.selected_list.min(visible_len - 1)
            } else {
                0
            };

            match error_msg {
                Some(msg) => app.set_status(msg),
                None if app.all_lists.is_empty() => app.set_status("Este board no tiene listas."),
                None => app.clear_status(),
            }
        }
        Err(e) => {
            app.set_status(format!("Error cargando listas: {}", e));
        }
    }
}

/// Hides the currently selected list (by name, case-insensitive) and persists
/// it to `config.toml`'s `[tui] hidden_lists`.
pub fn toggle_hide_current_list(app: &mut App) {
    let Some(list) = app.current_list().cloned() else {
        return;
    };
    app.hidden_lists.insert(list.name.to_lowercase());

    let visible_len = app.visible_lists().len();
    app.selected_list = if visible_len > 0 {
        app.selected_list.min(visible_len - 1)
    } else {
        0
    };

    match persist_hidden_lists(app) {
        Ok(()) => {
            app.set_status(format!("Lista oculta: {} ('V' para mostrar todas)", list.name));
        }
        Err(e) => {
            app.set_status(format!(
                "Lista oculta (no se pudo guardar en config.toml: {})",
                e
            ));
        }
    }
}

/// Clears the hidden-lists filter and persists it.
pub fn show_all_lists(app: &mut App) {
    app.hidden_lists.clear();
    match persist_hidden_lists(app) {
        Ok(()) => app.set_status("Todas las listas visibles."),
        Err(e) => {
            app.set_status(format!(
                "Mostrando todas (no se pudo guardar en config.toml: {})",
                e
            ))
        }
    }
}

fn persist_hidden_lists(app: &App) -> Result<()> {
    let mut config = crate::config::Config::load()?;
    config.tui.hidden_lists = app.hidden_lists.iter().cloned().collect();
    config.save()
}

pub async fn refresh_current_list(client: &HteamClient, app: &mut App) {
    let Some(list) = app.current_list().cloned() else {
        return;
    };
    match client.get_cards(list.id, Some(app.board_number)).await {
        Ok((cards, _)) => {
            app.cards_by_list.insert(list.id, cards);
        }
        Err(e) => {
            app.set_status(format!("Error recargando '{}': {}", list.name, e));
        }
    }
}

/// Optimistic move + revert-on-failure, mirroring the Neovim plugin's
/// `move_card_visually` pattern (ui.lua) so the UI feels instant.
pub async fn move_active_card(client: &HteamClient, app: &mut App, delta: i32) {
    let Some(from_list) = app.current_list().cloned() else {
        return;
    };
    let visible = app.visible_lists();
    let target_pos = app.selected_list as i32 + delta;
    if target_pos < 0 || target_pos as usize >= visible.len() {
        return;
    }
    let to_list = visible[target_pos as usize].clone();
    drop(visible);
    let idx = app.current_card_index();

    let Some(card) = app
        .cards_by_list
        .get(&from_list.id)
        .and_then(|cards| cards.get(idx))
        .cloned()
    else {
        return;
    };

    if let Some(cards) = app.cards_by_list.get_mut(&from_list.id) {
        cards.remove(idx);
    }
    app.cards_by_list
        .entry(to_list.id)
        .or_default()
        .push(card.clone());

    match client
        .move_card(card.id, from_list.id, to_list.id, Some(app.board_number))
        .await
    {
        Ok(()) => {
            let new_len = app.cards_by_list.get(&to_list.id).map(|c| c.len()).unwrap_or(1);
            app.selected_card.insert(to_list.id, new_len - 1);
            app.selected_list = target_pos as usize;
            app.set_status(format!("Movida: {} → {}", card.name, to_list.name));
        }
        Err(e) => {
            if let Some(cards) = app.cards_by_list.get_mut(&to_list.id) {
                if let Some(pos) = cards.iter().position(|c| c.id == card.id) {
                    cards.remove(pos);
                }
            }
            let insert_at = idx.min(
                app.cards_by_list
                    .get(&from_list.id)
                    .map(|c| c.len())
                    .unwrap_or(0),
            );
            app.cards_by_list
                .entry(from_list.id)
                .or_default()
                .insert(insert_at, card);
            app.set_status(format!("Error moviendo card: {}", e));
        }
    }
}

/// Opens the reminders popup and (re)loads pending reminders.
pub async fn open_reminders(client: &HteamClient, app: &mut App) {
    app.show_reminders = true;
    app.reminders_scroll = 0;
    match client.get_reminders().await {
        Ok(list) => app.reminders = list,
        Err(e) => app.set_status(format!("Error cargando reminders: {}", e)),
    }
}

pub fn close_reminders(app: &mut App) {
    app.show_reminders = false;
}

/// Creates a reminder for whichever card was selected on the board when the
/// popup was opened, then refreshes the popup's list.
pub async fn add_reminder_for_current_card(client: &HteamClient, app: &mut App) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };
    match client.set_reminder(card.id).await {
        Ok(()) => {
            app.set_status(format!("Reminder creado: {}", card.name));
            if let Ok(list) = client.get_reminders().await {
                app.reminders = list;
            }
        }
        Err(e) => {
            app.set_status(format!("Error creando reminder: {}", e));
        }
    }
}

/// Opens the comments popup for the currently selected card and loads its
/// existing comments. No-op if no card is selected.
pub async fn open_comments(client: &HteamClient, app: &mut App) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };
    app.show_comments = true;
    app.composing_comment = false;
    app.comment_input.clear();
    app.mention_suggestions.clear();
    app.comments_scroll = 0;

    match client.get_card_comments(card.id).await {
        Ok(list) => app.comments = list,
        Err(e) => app.set_status(format!("Error cargando comentarios: {}", e)),
    }
}

pub fn close_comments(app: &mut App) {
    app.show_comments = false;
    cancel_composing_comment(app);
}

pub fn start_composing_comment(app: &mut App) {
    app.composing_comment = true;
    app.comment_input.clear();
    app.mention_suggestions.clear();
}

pub fn cancel_composing_comment(app: &mut App) {
    app.composing_comment = false;
    app.comment_input.clear();
    app.mention_suggestions.clear();
}

pub async fn comment_input_push(client: &HteamClient, app: &mut App, ch: char) {
    app.comment_input.push(ch);
    update_mention_suggestions(client, app).await;
}

pub async fn comment_input_backspace(client: &HteamClient, app: &mut App) {
    app.comment_input.pop();
    update_mention_suggestions(client, app).await;
}

/// Fires a fresh `search_users` call on every keystroke that changes the
/// trailing `@word` — simple and consistent with the rest of the TUI (no
/// background task/debounce), at the cost of one request per character while
/// typing a mention.
async fn update_mention_suggestions(client: &HteamClient, app: &mut App) {
    match current_mention_query(&app.comment_input) {
        // Search on a bare '@' too (empty query) — matches the Neovim
        // plugin's behavior of firing on any `@([%w_]*)$` match, capture
        // included, so suggestions show up immediately instead of only
        // after the first letter.
        Some(query) => match client.search_users(query).await {
            Ok(list) => {
                app.mention_suggestions = list;
                app.mention_selected = 0;
            }
            Err(e) => {
                app.mention_suggestions.clear();
                app.set_status(format!("Error buscando usuarios: {}", e));
            }
        },
        None => app.mention_suggestions.clear(),
    }
}

/// Returns the partial username being typed if the text ends in an
/// unfinished `@mention` token (mirrors the Neovim plugin's `@([%w_]*)$`
/// end-of-line match).
fn current_mention_query(text: &str) -> Option<&str> {
    let word_start = text
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    text[word_start..].strip_prefix('@')
}

pub fn mention_move_selection(app: &mut App, delta: i32) {
    if app.mention_suggestions.is_empty() {
        return;
    }
    let len = app.mention_suggestions.len() as i32;
    let next = (app.mention_selected as i32 + delta).rem_euclid(len);
    app.mention_selected = next as usize;
}

/// Replaces the trailing `@partial` with `@username ` and closes the
/// suggestions dropdown.
pub fn accept_mention_suggestion(app: &mut App) {
    let Some(user) = app.mention_suggestions.get(app.mention_selected).cloned() else {
        return;
    };
    let word_start = app
        .comment_input
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    app.comment_input.truncate(word_start);
    app.comment_input.push('@');
    app.comment_input.push_str(&user.selected_text);
    app.comment_input.push(' ');
    app.mention_suggestions.clear();
}

pub async fn submit_comment(client: &HteamClient, app: &mut App) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };
    let text = app.comment_input.trim().to_string();
    if text.is_empty() {
        return;
    }

    match client.post_comment(card.id, &text, Some(app.board_number)).await {
        Ok(()) => {
            app.set_status("Comentario publicado.");
            cancel_composing_comment(app);
            if let Ok(list) = client.get_card_comments(card.id).await {
                app.comments = list;
            }
        }
        Err(e) => {
            app.set_status(format!("Error publicando comentario: {}", e));
        }
    }
}

/// `update_card_description` resolves board_number internally from the saved
/// config (not from `app.board_number`), so this only round-trips correctly
/// when the TUI is running against the user's configured default board — same
/// limitation the plain `hteam cards update-desc` command already has.
pub async fn save_description(client: &HteamClient, app: &mut App, card_id: u64, description: &str) {
    match client.update_card_description(card_id, description).await {
        Ok(()) => {
            app.set_status("Descripción actualizada.");
            refresh_current_list(client, app).await;
        }
        Err(e) => {
            app.set_status(format!("Error guardando descripción: {}", e));
        }
    }
}

/// Opens the description popup for the currently selected card, seeding the
/// input with its current description. No-op if no card is selected.
pub fn open_description(app: &mut App) {
    let Some(card) = app.current_card() else {
        return;
    };
    app.description_input = card.description.clone().unwrap_or_default();
    app.show_description = true;
}

pub fn close_description(app: &mut App) {
    app.show_description = false;
    app.description_input.clear();
}

pub fn description_input_push(app: &mut App, ch: char) {
    app.description_input.push(ch);
}

pub fn description_input_backspace(app: &mut App) {
    app.description_input.pop();
}

/// Saves the edited description (if the card is still resolvable) and closes
/// the popup regardless — mirrors `submit_comment`'s cancel-then-save order.
pub async fn submit_description(client: &HteamClient, app: &mut App) {
    let Some(card) = app.current_card().cloned() else {
        close_description(app);
        return;
    };
    let text = app.description_input.clone();
    close_description(app);
    save_description(client, app, card.id, &text).await;
}

/// Opens the projects popup. If nothing is active yet, auto-loads the most
/// recently used saved project (if any) so 'P' shows something useful right
/// away instead of an empty screen.
pub async fn open_projects(client: &HteamClient, app: &mut App) {
    app.show_projects = true;
    app.composing_project = false;
    app.project_input.clear();
    app.projects_detail_focused = false;
    app.projects_scroll = 0;

    if app.active_project.is_none() {
        if let Some(&id) = app.known_projects.first() {
            load_project(client, app, id).await;
        }
    }
}

pub fn close_projects(app: &mut App) {
    app.show_projects = false;
    app.composing_project = false;
    app.project_input.clear();
    app.projects_detail_focused = false;
}

pub fn start_composing_project(app: &mut App) {
    app.composing_project = true;
    app.project_input.clear();
}

pub fn cancel_composing_project(app: &mut App) {
    app.composing_project = false;
    app.project_input.clear();
}

pub fn project_input_push(app: &mut App, ch: char) {
    if ch.is_ascii_digit() {
        app.project_input.push(ch);
    }
}

pub fn project_input_backspace(app: &mut App) {
    app.project_input.pop();
}

pub fn move_project_selection(app: &mut App, delta: i32) {
    if app.known_projects.is_empty() {
        return;
    }
    let len = app.known_projects.len() as i32;
    let next = (app.selected_project_idx as i32 + delta).clamp(0, len - 1);
    app.selected_project_idx = next as usize;
}

/// Loads whatever project id was just typed in, then leaves compose mode.
pub async fn submit_project_input(client: &HteamClient, app: &mut App) {
    let Ok(id) = app.project_input.trim().parse::<u64>() else {
        app.set_status("Número de proyecto inválido.");
        return;
    };
    app.composing_project = false;
    app.project_input.clear();
    load_project(client, app, id).await;
}

/// Loads whichever saved project id is currently highlighted in the list.
pub async fn select_known_project(client: &HteamClient, app: &mut App) {
    let Some(&id) = app.known_projects.get(app.selected_project_idx) else {
        return;
    };
    load_project(client, app, id).await;
}

async fn load_project(client: &HteamClient, app: &mut App, id: u64) {
    app.active_project = Some(id);
    remember_project(app, id);

    match client.get_project_milestones(id).await {
        Ok(list) => app.project_milestones = list,
        Err(e) => app.set_status(format!("Error cargando milestones: {}", e)),
    }
    match client.get_project_tasks(id).await {
        Ok(resp) => app.project_tasks = resp.results,
        Err(e) => app.set_status(format!("Error cargando tasks: {}", e)),
    }
}

/// Moves `id` to the front of the known-projects MRU list (deduping) and
/// persists it to `config.toml`.
fn remember_project(app: &mut App, id: u64) {
    app.known_projects.retain(|&p| p != id);
    app.known_projects.insert(0, id);
    app.selected_project_idx = 0;

    if let Err(e) = persist_known_projects(app) {
        app.set_status(format!(
            "Proyecto cargado (no se pudo guardar en config.toml: {})",
            e
        ));
    }
}

fn persist_known_projects(app: &App) -> Result<()> {
    let mut config = crate::config::Config::load()?;
    config.tui.known_projects = app.known_projects.clone();
    config.save()
}

/// Starts composing a new card for the currently selected list. No-op if
/// there's no visible list to add to.
pub fn start_composing_card(app: &mut App) {
    let Some(list) = app.current_list() else {
        return;
    };
    let list_id = list.id;
    app.composing_card = true;
    app.new_card_input.clear();
    app.new_card_list_id = Some(list_id);
}

pub fn cancel_composing_card(app: &mut App) {
    app.composing_card = false;
    app.new_card_input.clear();
    app.new_card_list_id = None;
}

pub fn new_card_input_push(app: &mut App, ch: char) {
    app.new_card_input.push(ch);
}

pub fn new_card_input_backspace(app: &mut App) {
    app.new_card_input.pop();
}

/// `create_card` resolves board_id/board_number internally from the saved
/// config (not from `app.board_number`), same caveat as
/// `update_card_description` — only fully correct against the user's
/// configured default board.
pub async fn submit_new_card(client: &HteamClient, app: &mut App) {
    let Some(list_id) = app.new_card_list_id else {
        cancel_composing_card(app);
        return;
    };
    let name = app.new_card_input.trim().to_string();
    if name.is_empty() {
        app.set_status("El nombre de la card no puede estar vacío.");
        return;
    }

    match client.create_card(&name, Some(list_id), None).await {
        Ok(card) => {
            app.set_status(format!("Card creada: {}", card.name));
            cancel_composing_card(app);
            if let Ok((cards, _)) = client.get_cards(list_id, Some(app.board_number)).await {
                app.cards_by_list.insert(list_id, cards);
            }
        }
        Err(e) => {
            app.set_status(format!("Error creando card: {}", e));
        }
    }
}

async fn refresh_working_on(client: &HteamClient, app: &mut App) {
    // Non-fatal: keep the previous set on a transient error instead of
    // blanking out the border highlight everywhere.
    if let Ok(items) = client.get_working_on().await {
        app.working_on = items;
    }
}

async fn refresh_user_shift(client: &HteamClient, app: &mut App) {
    // Non-fatal: keep the previous value on a transient error instead of
    // blanking out the header.
    if let Ok(resume) = client.get_workshift_resume().await {
        app.user_shift = resume.last;
    }
}

/// Toggles "working on" for the currently selected card: stops it if it's
/// already the active one (using the working-on-it *record* id, not the card
/// id — `stop_working` needs the former), starts it otherwise.
pub async fn toggle_working_on(client: &HteamClient, app: &mut App) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };

    if let Some(working_id) = app.working_on_id_for(card.id) {
        match client.stop_working(working_id).await {
            Ok(()) => {
                app.set_status(format!("Dejaste de trabajar en: {}", card.name));
                refresh_working_on(client, app).await;
            }
            Err(e) => app.set_status(format!("Error deteniendo working on: {}", e)),
        }
    } else {
        match client.start_working(card.id).await {
            Ok(()) => {
                app.set_status(format!("Working on: {}", card.name));
                refresh_working_on(client, app).await;
            }
            Err(e) => app.set_status(format!("Error iniciando working on: {}", e)),
        }
    }
}

/// Scroll hacia abajo (`delta` > 0) o arriba (`delta` < 0) en la lista de comentarios.
/// Solo tiene efecto mientras se visualizan comentarios, no al componer.
pub fn scroll_comments(app: &mut App, delta: i32) {
    app.comments_scroll = (app.comments_scroll as i32 + delta).max(0) as u16;
}

/// Scroll hacia abajo (`delta` > 0) o arriba (`delta` < 0) en el popup de reminders.
pub fn scroll_reminders(app: &mut App, delta: i32) {
    app.reminders_scroll = (app.reminders_scroll as i32 + delta).max(0) as u16;
}

/// Scroll hacia abajo (`delta` > 0) o arriba (`delta` < 0) en el popup de ayuda.
pub fn scroll_help(app: &mut App, delta: i32) {
    app.help_scroll = (app.help_scroll as i32 + delta).max(0) as u16;
}

/// Alterna el foco entre la lista de proyectos guardados (izquierda) y el panel de detalle (derecha).
pub fn toggle_projects_focus(app: &mut App) {
    app.projects_detail_focused = !app.projects_detail_focused;
    app.projects_scroll = 0;
}

/// Scroll hacia abajo (`delta` > 0) o arriba (`delta` < 0) en el panel de detalle de proyectos.
pub fn scroll_projects(app: &mut App, delta: i32) {
    app.projects_scroll = (app.projects_scroll as i32 + delta).max(0) as u16;
}
