use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::client::HteamClient;
use crate::operations;

use super::app::{App, HelpContext, UpdateAvailableInfo};
use super::net::{self, JobOutcome, Jobs, WeeklyObjectivesLoaded};

/// Abre el popup de ayuda mostrando solo la sección de `ctx` — ver
/// `HelpContext`. El popup subyacente (si `ctx` no es `Board`) sigue abierto
/// detrás: cerrar la ayuda vuelve a él tal cual, no hace falta reabrirlo.
pub fn open_help(app: &mut App, ctx: HelpContext) {
    app.show_help = true;
    app.help_context = ctx;
    app.help_scroll.reset();
}

pub fn close_help(app: &mut App) {
    app.show_help = false;
}

pub fn request_refresh_all(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    net::request_board_refresh(client, app, jobs);
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
            app.set_status(format!(
                "Lista oculta: {} ('V' para mostrar todas)",
                list.name
            ));
        }
        Err(e) => {
            app.set_status(format!(
                "Lista oculta (no se pudo guardar en config.toml: {})",
                e
            ));
        }
    }
}

/// Open the `/` search bar (pre-filled with the current filter summary).
pub fn open_card_search(app: &mut App) {
    app.composing_search = true;
    let seed = app.card_filters_summary().unwrap_or_default();
    app.search_input.set(&seed);
    app.clear_status();
}

pub fn search_input_push(app: &mut App, c: char) {
    if c.is_control() {
        return;
    }
    app.search_input.push(c);
    apply_search_draft(app);
}

pub fn search_input_backspace(app: &mut App) {
    app.search_input.backspace();
    apply_search_draft(app);
}

/// Live-apply the draft query so results update while typing.
fn apply_search_draft(app: &mut App) {
    app.card_filters = crate::operations::cards::parse_search_query(app.search_input.as_str());
    app.clamp_card_selection();
}

/// Confirm search bar (keeps current filters) and leave compose mode.
pub fn confirm_card_search(app: &mut App) {
    apply_search_draft(app);
    app.composing_search = false;
    if let Some(summary) = app.card_filters_summary() {
        app.set_status(format!("Filtro: {} · X limpiar", summary));
    } else {
        app.set_status("Filtro vacío (todas las cards).");
    }
}

/// Cancel compose without clearing an already-applied filter; Esc while
/// composing with empty input clears filters.
pub fn cancel_card_search(app: &mut App) {
    app.composing_search = false;
    if app.search_input.as_str().trim().is_empty() {
        clear_card_search(app);
    } else {
        // Re-parse last draft so board stays consistent with the bar content.
        apply_search_draft(app);
        if let Some(summary) = app.card_filters_summary() {
            app.set_status(format!("Filtro: {} · X limpiar", summary));
        }
    }
}

pub fn clear_card_search(app: &mut App) {
    app.composing_search = false;
    app.search_input.clear();
    app.card_filters = crate::operations::cards::CardSearchFilters::default();
    app.clamp_card_selection();
    app.set_status("Filtro limpiado.");
}

/// Clears the hidden-lists filter and persists it.
pub fn show_all_lists(app: &mut App) {
    app.hidden_lists.clear();
    match persist_hidden_lists(app) {
        Ok(()) => app.set_status("Todas las listas visibles."),
        Err(e) => app.set_status(format!(
            "Mostrando todas (no se pudo guardar en config.toml: {})",
            e
        )),
    }
}

fn persist_hidden_lists(app: &App) -> Result<()> {
    let mut config = crate::config::Config::load()?;
    config.tui.hidden_lists = app.hidden_lists.iter().cloned().collect();
    config.save()
}

/// Optimistic move + revert-on-failure, mirroring the Neovim plugin's
/// `move_card_visually` pattern (ui.lua) so the UI feels instant. The HTTP
/// call runs in the background; a late failure still reverts if the request
/// id matches.
pub fn move_active_card(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs, delta: i32) {
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

    // Use the filtered view's selected card (by id), not a raw list index —
    // with `/` search active the view index differs from cards_by_list.
    let Some(card) = app.current_card().cloned() else {
        return;
    };
    let from_idx = app
        .cards_by_list
        .get(&from_list.id)
        .and_then(|cards| cards.iter().position(|c| c.id == card.id));
    let Some(from_idx) = from_idx else {
        return;
    };

    if let Some(cards) = app.cards_by_list.get_mut(&from_list.id) {
        cards.remove(from_idx);
    }
    app.cards_by_list
        .entry(to_list.id)
        .or_default()
        .push(card.clone());
    app.clamp_card_selection();

    let id = app.requests.bump_move_card();
    let board_number = app.board_number;
    let from_list_id = from_list.id;
    let to_list_id = to_list.id;
    let to_list_name = to_list.name.clone();
    let card_id = card.id;
    let target_list_pos = target_pos as usize;

    jobs.spawn(async move {
        let result = operations::cards::move_card(
            &client,
            card_id,
            Some(from_list_id),
            to_list_id,
            Some(board_number),
        )
        .await
        .map_err(|e| e.to_string());
        JobOutcome::MoveCard {
            id,
            board_number,
            card,
            from_list_id,
            to_list_id,
            from_idx,
            target_list_pos,
            to_list_name,
            result,
        }
    });
}

/// Opens the reminders popup and (re)loads pending reminders.
pub fn open_reminders(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    app.show_reminders = true;
    app.reminders_scroll.reset();
    let id = app.requests.bump_reminders();
    app.begin_busy("Cargando reminders...");
    jobs.spawn(async move {
        let result = operations::reminders::list(&client)
            .await
            .map_err(|e| e.to_string());
        JobOutcome::Reminders { id, result }
    });
}

pub fn close_reminders(app: &mut App) {
    app.show_reminders = false;
}

pub fn open_weekly_objectives(app: &mut App, jobs: &Jobs, force: bool) {
    app.show_weekly_objectives = true;
    app.weekly_objectives_scroll.reset();
    app.weekly_objectives_error = None;
    let id = app.requests.bump_objectives();
    app.begin_busy(if force {
        "Refrescando objetivos semanales..."
    } else {
        "Cargando objetivos semanales..."
    });

    jobs.spawn(async move {
        let result = load_weekly_objectives(force)
            .await
            .map(|view| WeeklyObjectivesLoaded {
                set: view.set,
                from_cache: view.from_cache,
                synced_at: view.synced_at,
            })
            .map_err(|e| e.to_string());
        JobOutcome::WeeklyObjectives { id, result }
    });
}

async fn load_weekly_objectives(
    force: bool,
) -> Result<operations::objectives::WeeklyObjectivesView> {
    let mut config = crate::config::Config::load()?;
    let view = operations::objectives::show(&mut config, force).await?;
    if !view.from_cache {
        config.save()?;
    }
    Ok(view)
}

pub fn close_weekly_objectives(app: &mut App) {
    app.show_weekly_objectives = false;
}

/// Opens the one-shot post-update changelog popup with the extracted markdown.
pub fn open_startup_changelog(app: &mut App, text: String) {
    app.startup_changelog = Some(text);
    app.show_changelog = true;
    app.changelog_scroll.reset();
}

pub fn close_startup_changelog(app: &mut App) {
    app.show_changelog = false;
    app.startup_changelog = None;
    app.changelog_scroll.reset();
    // If a newer release was also noticed at startup, show that popup next.
    if app.update_available.is_some() {
        app.show_update_available = true;
    }
}

/// Copies the public CHANGELOG.md URL so the user can open the full history.
pub fn copy_changelog_link(app: &mut App) {
    match operations::clipboard::copy(operations::update::CHANGELOG_URL) {
        Ok(()) => app.set_status(format!("{} Changelog link copiado", operations::COPY_ICON)),
        Err(e) => app.set_status(format!("Error copiando el link: {}", e)),
    }
}

pub fn open_update_available(app: &mut App, info: UpdateAvailableInfo) {
    app.update_available = Some(info);
    // Don't stack on top of the changelog; open_startup_changelog close chains it.
    if !app.show_changelog {
        app.show_update_available = true;
    }
}

pub fn close_update_available(app: &mut App) {
    app.show_update_available = false;
    app.update_available = None;
}

/// Copies the GitHub releases page URL from the Update Available popup.
pub fn copy_releases_link(app: &mut App) {
    match operations::clipboard::copy(operations::update::RELEASES_URL) {
        Ok(()) => app.set_status(format!("{} Releases link copiado", operations::COPY_ICON)),
        Err(e) => app.set_status(format!("Error copiando el link: {}", e)),
    }
}

/// Creates a reminder for whichever card was selected on the board when the
/// popup was opened, then refreshes the popup's list.
pub fn add_reminder_for_current_card(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };
    let id = app.requests.bump_reminders();
    let card_id = card.id;
    let card_name = card.name.clone();
    app.begin_busy("Creando reminder...");
    jobs.spawn(async move {
        let result = match operations::reminders::create(&client, card_id).await {
            Ok(()) => operations::reminders::list(&client)
                .await
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        JobOutcome::ReminderCreated {
            id,
            card_name,
            result,
        }
    });
}

/// Opens the comments popup for the currently selected card and loads its
/// existing comments. No-op if no card is selected.
pub fn open_comments(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };
    app.show_comments = true;
    app.composing_comment = false;
    app.comment_input.clear();
    app.mention_suggestions.clear();
    app.comments_scroll.reset();
    app.current_follow_up = None;
    app.comments.clear();

    let id = app.requests.bump_comments();
    let card_id = card.id;
    let board_number = app.board_number;
    let has_follow_up = card.has_follow_up();
    app.begin_busy("Cargando comentarios...");

    jobs.spawn(async move {
        let comments = operations::comments::list_comments(&client, card_id)
            .await
            .map_err(|e| e.to_string());
        let follow_up = if has_follow_up {
            operations::comments::get_follow_up(&client, card_id, Some(board_number))
                .await
                .map_err(|e| e.to_string())
        } else {
            Ok(None)
        };
        JobOutcome::Comments {
            id,
            card_id,
            comments,
            follow_up,
        }
    });
}

pub fn close_comments(app: &mut App) {
    app.show_comments = false;
    app.current_follow_up = None;
    // Invalidate in-flight comment loads for this popup.
    app.requests.bump_comments();
    cancel_composing_comment(app);
}

/// Completa el seguimiento mostrado en el popup de comentarios y refresca la
/// lista actual para que la card pierda el indicador de urgencia en el board.
pub fn complete_current_follow_up(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    resolve_current_follow_up(client, app, jobs, true);
}

/// Cancela (elimina) el seguimiento mostrado en el popup de comentarios y
/// refresca la lista actual para que la card pierda el indicador de urgencia
/// en el board.
pub fn cancel_current_follow_up(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    resolve_current_follow_up(client, app, jobs, false);
}

fn resolve_current_follow_up(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs, complete: bool) {
    let Some(follow_up) = app.current_follow_up.clone() else {
        return;
    };
    let Some(list) = app.current_list().cloned() else {
        return;
    };
    let id = app.requests.bump_follow_up();
    let board_number = app.board_number;
    let list_id = list.id;
    let follow_up_id = follow_up.id;
    app.begin_busy(if complete {
        "Completando seguimiento..."
    } else {
        "Cancelando seguimiento..."
    });

    jobs.spawn(async move {
        let result = if complete {
            operations::comments::complete_follow_up(&client, follow_up_id).await
        } else {
            operations::comments::cancel_follow_up(&client, follow_up_id).await
        };
        let (result, cards) = match result {
            Ok(()) => {
                let cards = operations::cards::list_cards(&client, list_id, Some(board_number))
                    .await
                    .map(|(c, _)| c)
                    .map_err(|e| e.to_string());
                (Ok(()), Some(cards))
            }
            Err(e) => (Err(e.to_string()), None),
        };
        JobOutcome::FollowUp {
            id,
            board_number,
            list_id,
            complete,
            result,
            cards,
        }
    });
}

pub fn start_composing_comment(app: &mut App) {
    app.composing_comment = true;
    app.comment_input.clear();
    app.comment_follow = false;
    app.comment_date_input.clear();
    app.comment_date_focused = false;
    app.mention_suggestions.clear();
}

pub fn cancel_composing_comment(app: &mut App) {
    app.composing_comment = false;
    app.comment_input.clear();
    app.comment_follow = false;
    app.comment_date_input.clear();
    app.comment_date_focused = false;
    clear_mention_state(app);
}

pub fn toggle_comment_follow(app: &mut App) {
    app.comment_follow = !app.comment_follow;
}

pub fn toggle_comment_date_focus(app: &mut App) {
    app.comment_date_focused = !app.comment_date_focused;
}

/// Restricted to the characters `COMMENT_DATE_FORMAT` ("%Y-%m-%d %H:%M")
/// can ever contain, so the field can't hold something `resolve_comment_date`
/// would always reject.
pub fn comment_date_input_push(app: &mut App, ch: char) {
    if ch.is_ascii_digit() || ch == '-' || ch == ':' || ch == ' ' {
        app.comment_date_input.push(ch);
    }
}

pub fn comment_date_input_backspace(app: &mut App) {
    app.comment_date_input.backspace();
}

/// Pause after the last keystroke before firing `search_users`. Short enough
/// to feel snappy on bare `@`, long enough to collapse a burst of typing into
/// one request.
pub const MENTION_DEBOUNCE: Duration = Duration::from_millis(200);

pub fn comment_input_push(app: &mut App, ch: char) {
    app.comment_input.push(ch);
    schedule_mention_suggestions(app);
}

pub fn comment_input_backspace(app: &mut App) {
    app.comment_input.backspace();
    schedule_mention_suggestions(app);
}

/// On each keystroke that touches a trailing `@token`, invalidate in-flight
/// mention searches and (re)arm the debounce timer. Actual HTTP is fired by
/// [`poll_mention_debounce`] once the pause elapses — so a burst of typing
/// collapses to one request without blocking the keyboard.
pub fn schedule_mention_suggestions(app: &mut App) {
    // Always bump: cancels any in-flight Mentions job and any armed debounce
    // that would have used the previous id.
    app.requests.bump_mentions();

    match operations::comments::mention_query(app.comment_input.as_str()) {
        Some(_) => {
            app.mention_debounce_at = Some(Instant::now() + MENTION_DEBOUNCE);
        }
        None => {
            app.mention_debounce_at = None;
            app.mention_suggestions.clear();
        }
    }
}

/// If a mention debounce window has elapsed, spawn a single search for the
/// current `@token`. No-op while the timer is still running or there is no
/// active mention query (e.g. user deleted the `@`).
pub fn poll_mention_debounce(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(deadline) = app.mention_debounce_at else {
        return;
    };
    if Instant::now() < deadline {
        return;
    }
    app.mention_debounce_at = None;

    let Some(query) = operations::comments::mention_query(app.comment_input.as_str()) else {
        app.mention_suggestions.clear();
        return;
    };
    let query = query.to_string();
    // Use the id from the last schedule (already bumped); do not bump again
    // or a concurrent schedule would race against this fire.
    let id = app.requests.mentions;
    jobs.spawn(async move {
        let result = operations::comments::mention_suggestions(&client, &query)
            .await
            .map_err(|e| e.to_string());
        JobOutcome::Mentions { id, query, result }
    });
}

/// Clears suggestions, cancels debounce, and invalidates in-flight searches.
pub fn clear_mention_state(app: &mut App) {
    app.requests.bump_mentions();
    app.mention_debounce_at = None;
    app.mention_suggestions.clear();
    app.mention_selected = 0;
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
        .as_str()
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    app.comment_input.truncate(word_start);
    app.comment_input.push('@');
    app.comment_input.push_str(&user.selected_text);
    app.comment_input.push(' ');
    clear_mention_state(app);
}

pub fn submit_comment(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };
    let text = app.comment_input.trim().to_string();
    if text.is_empty() {
        return;
    }

    let date_input = app.comment_date_input.trim().to_string();
    let date = if date_input.is_empty() {
        None
    } else {
        Some(date_input)
    };
    let follow = app.comment_follow;
    let board_number = app.board_number;
    let card_id = card.id;
    let id = app.requests.bump_comment_post();
    app.begin_busy("Publicando comentario...");

    jobs.spawn(async move {
        let result = match operations::comments::post_comment(
            &client,
            card_id,
            &text,
            Some(board_number),
            follow,
            date.as_deref(),
        )
        .await
        {
            Ok(()) => operations::comments::list_comments(&client, card_id)
                .await
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        JobOutcome::CommentPosted {
            id,
            card_id,
            result,
        }
    });
}

/// Opens the description popup for the currently selected card, seeding the
/// input with its current description. No-op if no card is selected.
pub fn open_description(app: &mut App) {
    let Some(card) = app.current_card() else {
        return;
    };
    app.description_input
        .set(card.description.clone().unwrap_or_default());
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
    app.description_input.backspace();
}

/// Saves the edited description (if the card is still resolvable) and closes
/// the popup regardless — mirrors `submit_comment`'s cancel-then-save order.
pub fn submit_description(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(card) = app.current_card().cloned() else {
        close_description(app);
        return;
    };
    let Some(list) = app.current_list().cloned() else {
        close_description(app);
        return;
    };
    let text = app.description_input.clone();
    close_description(app);

    let id = app.requests.bump_description();
    let board_number = app.board_number;
    let list_id = list.id;
    let card_id = card.id;
    app.begin_busy("Guardando descripción...");

    jobs.spawn(async move {
        let result =
            match operations::cards::update_description(&client, card_id, text.as_str()).await {
                Ok(()) => {
                    let cards = operations::cards::list_cards(&client, list_id, Some(board_number))
                        .await
                        .map(|(c, _)| c)
                        .map_err(|e| e.to_string());
                    (Ok(()), Some(cards))
                }
                Err(e) => (Err(e.to_string()), None),
            };
        JobOutcome::DescriptionSaved {
            id,
            board_number,
            list_id,
            result: result.0,
            cards: result.1,
        }
    });
}

/// Opens the projects popup. If nothing is active yet, auto-loads the most
/// recently used saved project (if any) so 'P' shows something useful right
/// away instead of an empty screen.
pub fn open_projects(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    app.show_projects = true;
    app.composing_project = false;
    app.project_input.clear();
    app.projects_detail_focused = false;
    app.projects_scroll.reset();

    if app.active_project.is_none() {
        if let Some(&id) = app.known_projects.first() {
            load_project(client, app, jobs, id);
        }
    }
}

pub fn close_projects(app: &mut App) {
    app.show_projects = false;
    app.composing_project = false;
    app.project_input.clear();
    app.projects_detail_focused = false;
    app.show_live_projects = false;
    app.requests.bump_projects();
    app.requests.bump_live_projects();
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
    app.project_input.backspace();
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
pub fn submit_project_input(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Ok(id) = app.project_input.trim().parse::<u64>() else {
        app.set_status("Número de proyecto inválido.");
        return;
    };
    app.composing_project = false;
    app.project_input.clear();
    load_project(client, app, jobs, id);
}

/// Loads whichever saved project id is currently highlighted in the list.
pub fn select_known_project(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(&id) = app.known_projects.get(app.selected_project_idx) else {
        return;
    };
    load_project(client, app, jobs, id);
}

/// Copia el link del proyecto resaltado en el popup al portapapeles ('c') —
/// reutiliza `operations::projects::project_url`, la misma función que
/// expone `hteam project link`. No depende de la red: no hace falta que el
/// proyecto ya esté cargado (`active_project`), solo que esté resaltado, sea
/// en `known_projects` o en `live_projects` (ver `show_live_projects`).
pub fn copy_project_link(client: &HteamClient, app: &mut App) {
    let id = if app.show_live_projects {
        let Some(project) = app.live_projects.get(app.selected_live_project_idx) else {
            return;
        };
        project.id
    } else {
        let Some(&id) = app.known_projects.get(app.selected_project_idx) else {
            return;
        };
        id
    };

    let url = operations::projects::project_url(client, id);
    match operations::clipboard::copy(&url) {
        Ok(()) => app.set_status(format!("{} Link copiado: {}", operations::COPY_ICON, url)),
        Err(e) => app.set_status(format!("Error copiando el link: {}", e)),
    }
}

/// Trae/oculta el listado en vivo de proyectos ('L') — reutiliza
/// `operations::projects::list`, la misma función que expone `hteam project
/// list`, filtrado por lo que haya quedado seleccionado en
/// `live_projects_status_idx` (por default, "Execution"). Al volver a
/// presionar 'L' con el listado abierto, lo cierra sin volver a pedirlo.
pub fn toggle_live_projects(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    if app.show_live_projects {
        app.show_live_projects = false;
        app.requests.bump_live_projects();
        return;
    }
    fetch_live_projects(client, app, jobs, true);
}

pub fn move_live_project_selection(app: &mut App, delta: i32) {
    if app.live_projects.is_empty() {
        return;
    }
    let len = app.live_projects.len() as i32;
    let next = (app.selected_live_project_idx as i32 + delta).clamp(0, len - 1);
    app.selected_live_project_idx = next as usize;
}

/// Cicla el filtro de estado del listado en vivo ('h'/'l', mismo orden que
/// los radios de `/projects/` en el sitio: All → Planning → Execution →
/// Finished → Cancelled → All) y vuelve a pedir la lista con el nuevo
/// filtro.
pub fn cycle_live_projects_status(
    client: Arc<HteamClient>,
    app: &mut App,
    jobs: &Jobs,
    delta: i32,
) {
    let len = operations::projects::PROJECT_STATUSES.len() as i32;
    let next = (app.live_projects_status_idx as i32 + delta).rem_euclid(len);
    app.live_projects_status_idx = next as usize;
    fetch_live_projects(client, app, jobs, false);
}

fn fetch_live_projects(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs, open: bool) {
    let status_idx = app.live_projects_status_idx;
    let status = operations::projects::PROJECT_STATUSES[status_idx];
    let id = app.requests.bump_live_projects();
    app.begin_busy("Cargando proyectos...");
    jobs.spawn(async move {
        let result = operations::projects::list(&client, Some(status))
            .await
            .map_err(|e| e.to_string());
        JobOutcome::LiveProjects {
            id,
            status_idx,
            result,
            open,
        }
    });
}

/// Carga el proyecto resaltado en el listado en vivo y vuelve a la vista
/// normal del popup (mismo efecto que elegir uno de `known_projects`: queda
/// como `active_project` y se agrega al MRU vía `remember_project`).
pub fn select_live_project(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(id) = app
        .live_projects
        .get(app.selected_live_project_idx)
        .map(|p| p.id)
    else {
        return;
    };
    load_project(client, app, jobs, id);
    app.show_live_projects = false;
}

fn load_project(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs, project_id: u64) {
    app.active_project = Some(project_id);
    remember_project(app, project_id);
    app.project_milestones.clear();
    app.project_tasks.clear();

    let id = app.requests.bump_projects();
    app.begin_busy("Cargando proyecto...");
    jobs.spawn(async move {
        let milestones = operations::projects::milestones(&client, project_id)
            .await
            .map_err(|e| e.to_string());
        let tasks = operations::projects::tasks(&client, project_id)
            .await
            .map(|resp| resp.results)
            .map_err(|e| e.to_string());
        JobOutcome::ProjectLoaded {
            id,
            project_id,
            milestones,
            tasks,
        }
    });
}

/// Moves `id` to the front of the known-projects MRU list (deduping) and
/// persists it to `config.toml`.
fn remember_project(app: &mut App, id: u64) {
    app.known_projects.retain(|&p| p != id);
    app.known_projects.insert(0, id);
    app.selected_project_idx = 0;

    if let Err(e) = persist_known_project(id) {
        app.set_status(format!(
            "Proyecto cargado (no se pudo guardar en config.toml: {})",
            e
        ));
    }
}

fn persist_known_project(id: u64) -> Result<()> {
    let mut config = crate::config::Config::load()?;
    operations::projects::remember_project(&mut config, id)
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
    app.new_card_input.backspace();
}

/// `create_card` resolves board_id/board_number from the client's cached
/// config, kept in sync with `app.board_number` by `switch_to_board` via
/// `client.set_board_number`, so the card always lands on the board
/// currently active in the TUI.
pub fn submit_new_card(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(list_id) = app.new_card_list_id else {
        cancel_composing_card(app);
        return;
    };
    let name = app.new_card_input.trim().to_string();
    if name.is_empty() {
        app.set_status("El nombre de la card no puede estar vacío.");
        return;
    }

    let id = app.requests.bump_new_card();
    let board_number = app.board_number;
    app.begin_busy("Creando card...");
    jobs.spawn(async move {
        let result = match operations::cards::create_card(&client, &name, Some(list_id)).await {
            Ok(card) => {
                match operations::cards::list_cards(&client, list_id, Some(board_number)).await {
                    Ok((cards, _)) => Ok((card.name, cards)),
                    Err(e) => Err(e.to_string()),
                }
            }
            Err(e) => Err(e.to_string()),
        };
        JobOutcome::CardCreated {
            id,
            board_number,
            list_id,
            result,
        }
    });
}

/// Copia el link de la card seleccionada al portapapeles ('c') — reutiliza
/// `operations::cards::card_url` y `operations::clipboard::copy`, las mismas
/// funciones que expone `hteam cards link`.
pub fn copy_card_link(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };

    let id = app.requests.bump_card_link();
    let board_number = app.board_number;
    let card_id = card.id;
    app.begin_busy("Obteniendo link...");
    jobs.spawn(async move {
        let result = operations::cards::card_url(&client, card_id, Some(board_number))
            .await
            .map_err(|e| e.to_string());
        JobOutcome::CardLink { id, result }
    });
}

/// Toggles "working on" for the currently selected card: stops it if it's
/// already the active one (using the working-on-it *record* id, not the card
/// id — `stop_working` needs the former), starts it otherwise.
pub fn toggle_working_on(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };

    let id = app.requests.bump_working();
    let card_name = card.name.clone();
    let working_id = app.working_on_id_for(card.id);
    let card_id = card.id;
    app.begin_busy(if working_id.is_some() {
        "Deteniendo working on..."
    } else {
        "Iniciando working on..."
    });

    jobs.spawn(async move {
        let outcome = if let Some(working_id) = working_id {
            match operations::working::stop_working(&client, working_id).await {
                Ok(()) => {
                    let working_on = operations::working::list_working(&client).await.ok();
                    JobOutcome::WorkingToggled {
                        id,
                        message: format!("Dejaste de trabajar en: {}", card_name),
                        working_on,
                        error: None,
                    }
                }
                Err(e) => JobOutcome::WorkingToggled {
                    id,
                    message: String::new(),
                    working_on: None,
                    error: Some(format!("Error deteniendo working on: {}", e)),
                },
            }
        } else {
            match operations::working::start_working(&client, card_id).await {
                Ok(()) => {
                    let working_on = operations::working::list_working(&client).await.ok();
                    JobOutcome::WorkingToggled {
                        id,
                        message: format!("Working on: {}", card_name),
                        working_on,
                        error: None,
                    }
                }
                Err(e) => JobOutcome::WorkingToggled {
                    id,
                    message: String::new(),
                    working_on: None,
                    error: Some(format!("Error iniciando working on: {}", e)),
                },
            }
        };
        outcome
    });
}

// ── Board-switch popup ──────────────────────────────────────────────────

/// Abre el popup de cambio de board y obtiene la lista desde la API.
pub fn open_board_switch(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    app.show_board_switch = true;
    app.available_boards.clear();
    app.selected_board_idx = 0;
    let id = app.requests.bump_boards_popup();
    let current_board = app.board_number;
    app.begin_busy("Cargando boards...");

    jobs.spawn(async move {
        let result = operations::boards::list_live(&client)
            .await
            .map_err(|e| e.to_string());
        JobOutcome::BoardsLoaded {
            id,
            current_board,
            result,
        }
    });
}

pub fn close_board_switch(app: &mut App) {
    app.show_board_switch = false;
    app.available_boards.clear();
    app.requests.bump_boards_popup();
}

pub fn move_board_selection(app: &mut App, delta: i32) {
    if app.available_boards.is_empty() {
        return;
    }
    let len = app.available_boards.len() as i32;
    let next = (app.selected_board_idx as i32 + delta).clamp(0, len - 1);
    app.selected_board_idx = next as usize;
}

/// Cambia al board actualmente resaltado en el popup.
pub fn select_current_board(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(board) = app.available_boards.get(app.selected_board_idx).cloned() else {
        return;
    };
    switch_to_board(client, app, jobs, board.id);
}

/// Lógica central del cambio de board: actualiza `board_number` (tanto en
/// `App` como en el `HteamClient`), limpia el estado del tablero, persiste
/// en config.toml y recarga las listas/cards en background.
fn switch_to_board(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs, id: u64) {
    if id == app.board_number {
        close_board_switch(app);
        return;
    }

    app.board_number = id;
    app.all_lists.clear();
    app.cards_by_list.clear();
    app.selected_card.clear();
    app.selected_list = 0;
    app.working_on.clear();

    close_board_switch(app);

    if let Err(e) = persist_board_switch(id) {
        app.set_status(format!(
            "Board #{} cargando (no se pudo guardar config: {})",
            id, e
        ));
    }

    let refresh_id = app.requests.bump_board_refresh();
    app.begin_busy("Cargando...");
    jobs.spawn(async move {
        // Sin esto, create_card/update_card_description seguirían resolviendo
        // el board desde la copia de Config cacheada al arrancar el cliente,
        // operando sobre el board viejo aunque la UI ya muestre el nuevo.
        client.set_board_number(id).await;
        let (lists, cards, working_on, user_shift) = net::fetch_board_snapshot(&client, id).await;
        JobOutcome::BoardRefresh {
            id: refresh_id,
            board_number: id,
            lists,
            cards,
            working_on,
            user_shift,
        }
    });
}

fn persist_board_switch(id: u64) -> Result<()> {
    let mut config = crate::config::Config::load()?;
    operations::boards::switch_board(&mut config, id, None)?;
    Ok(())
}

/// Alterna el foco entre la lista de proyectos guardados (izquierda) y el panel de detalle (derecha).
pub fn toggle_projects_focus(app: &mut App) {
    app.projects_detail_focused = !app.projects_detail_focused;
    app.projects_scroll.reset();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkingHoursConfig;
    use crate::tui::theme::Theme;
    use std::collections::HashSet;
    use std::time::Duration;

    fn test_app() -> App {
        App::new(
            1,
            HashSet::new(),
            Vec::new(),
            WorkingHoursConfig::default(),
            Theme::default(),
        )
    }

    #[test]
    fn schedule_arms_debounce_for_at_token() {
        let mut app = test_app();
        app.composing_comment = true;
        app.comment_input.set("hola @an");

        schedule_mention_suggestions(&mut app);

        assert!(app.mention_debounce_at.is_some());
        assert_eq!(app.requests.mentions, 1);
    }

    #[test]
    fn schedule_clears_when_no_at_token() {
        let mut app = test_app();
        app.composing_comment = true;
        app.comment_input.set("hola ana");
        app.mention_suggestions = vec![]; // placeholder; clear path must not panic
        app.mention_debounce_at = Some(Instant::now() + Duration::from_secs(5));

        schedule_mention_suggestions(&mut app);

        assert!(app.mention_debounce_at.is_none());
        assert!(app.mention_suggestions.is_empty());
    }

    #[test]
    fn reschedule_bumps_id_and_moves_deadline() {
        let mut app = test_app();
        app.comment_input.set("@a");
        schedule_mention_suggestions(&mut app);
        let first_id = app.requests.mentions;
        let first_at = app.mention_debounce_at.expect("armed");

        std::thread::sleep(Duration::from_millis(5));
        app.comment_input.set("@ab");
        schedule_mention_suggestions(&mut app);

        assert!(app.requests.mentions > first_id);
        assert!(app.mention_debounce_at.expect("re-armed") > first_at);
    }

    #[test]
    fn poll_before_deadline_does_not_clear_timer() {
        let mut app = test_app();
        app.comment_input.set("@an");
        schedule_mention_suggestions(&mut app);
        // Force a far-future deadline so poll is a no-op without sleeping 200ms.
        app.mention_debounce_at = Some(Instant::now() + Duration::from_secs(30));

        let (jobs, mut rx) = Jobs::channel();
        // Client is unused when poll returns early; skip building a real one.
        // We only assert the timer stays and no job is enqueued — but poll
        // needs a client Arc. Build a throwaway via Config::default.
        let client = Arc::new(
            crate::client::HteamClient::new(crate::config::Config::default()).expect("client"),
        );
        poll_mention_debounce(client, &mut app, &jobs);

        assert!(app.mention_debounce_at.is_some());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn poll_after_deadline_clears_timer_and_enqueues_job() {
        let mut app = test_app();
        app.comment_input.set("@an");
        schedule_mention_suggestions(&mut app);
        app.mention_debounce_at = Some(Instant::now() - Duration::from_millis(1));

        let client = Arc::new(
            crate::client::HteamClient::new(crate::config::Config::default()).expect("client"),
        );
        let (jobs, mut rx) = Jobs::channel();
        poll_mention_debounce(client, &mut app, &jobs);

        assert!(app.mention_debounce_at.is_none());
        // Drop the receiver so the spawned HTTP task's send is ignored; we
        // only care that poll cleared the timer and attempted a job.
        drop(rx.try_recv());
        let _ = &mut rx;
    }

    #[test]
    fn clear_mention_state_cancels_debounce() {
        let mut app = test_app();
        app.comment_input.set("@x");
        schedule_mention_suggestions(&mut app);
        clear_mention_state(&mut app);
        assert!(app.mention_debounce_at.is_none());
        assert!(app.mention_suggestions.is_empty());
    }

    #[test]
    fn open_startup_changelog_shows_popup_with_text() {
        let mut app = test_app();
        open_startup_changelog(&mut app, "## [0.9.0]\n\n- something".into());

        assert!(app.show_changelog);
        assert_eq!(
            app.startup_changelog.as_deref(),
            Some("## [0.9.0]\n\n- something")
        );
        assert_eq!(app.changelog_scroll.offset(), 0);
    }

    #[test]
    fn close_startup_changelog_clears_popup_state() {
        let mut app = test_app();
        open_startup_changelog(&mut app, "notes".into());
        app.changelog_scroll.by(3);

        close_startup_changelog(&mut app);

        assert!(!app.show_changelog);
        assert!(app.startup_changelog.is_none());
        assert_eq!(app.changelog_scroll.offset(), 0);
        assert!(!app.show_update_available);
    }

    #[test]
    fn close_changelog_opens_pending_update_available() {
        let mut app = test_app();
        open_startup_changelog(&mut app, "notes".into());
        open_update_available(
            &mut app,
            UpdateAvailableInfo {
                current_version: "0.9.0".into(),
                latest_version: "0.10.0".into(),
            },
        );
        assert!(app.show_changelog);
        assert!(
            !app.show_update_available,
            "update popup waits until changelog closes"
        );

        close_startup_changelog(&mut app);

        assert!(!app.show_changelog);
        assert!(app.show_update_available);
        assert_eq!(
            app.update_available
                .as_ref()
                .map(|i| i.latest_version.as_str()),
            Some("0.10.0")
        );
    }

    #[test]
    fn open_update_available_alone_shows_popup() {
        let mut app = test_app();
        open_update_available(
            &mut app,
            UpdateAvailableInfo {
                current_version: "0.9.0".into(),
                latest_version: "0.10.0".into(),
            },
        );

        assert!(app.show_update_available);
        close_update_available(&mut app);
        assert!(!app.show_update_available);
        assert!(app.update_available.is_none());
    }

    fn board_with_cards() -> App {
        use crate::models::{Card, Label, List};
        let mut app = test_app();
        app.all_lists = vec![List {
            id: 10,
            name: "DOING".into(),
            card_count: Some(2),
            color: None,
            position: None,
            show_url: None,
        }];
        let card = |id, name: &str| Card {
            id,
            name: name.into(),
            description: None,
            status: None,
            labels: vec![Label {
                id: 1,
                name: "DOING".into(),
                color: "#0f0".into(),
            }],
            number: None,
            title_url: None,
            subtitle_url: None,
            responsible: Some(serde_json::json!({"username": "ana"})),
            priority: None,
            worker: None,
            time_status: None,
        };
        app.cards_by_list
            .insert(10, vec![card(1, "Alpha GBI"), card(2, "Other task")]);
        app
    }

    #[test]
    fn card_search_filters_view_and_clears() {
        let mut app = board_with_cards();
        open_card_search(&mut app);
        assert!(app.composing_search);
        for c in "GBI".chars() {
            search_input_push(&mut app, c);
        }
        assert_eq!(app.view_cards(10).len(), 1);
        assert_eq!(app.view_cards(10)[0].id, 1);
        confirm_card_search(&mut app);
        assert!(!app.composing_search);
        assert!(app.card_filters_summary().unwrap().contains("GBI"));

        clear_card_search(&mut app);
        assert!(app.card_filters.is_empty());
        assert_eq!(app.view_cards(10).len(), 2);
    }

    #[test]
    fn card_search_at_user_and_hash_label() {
        let mut app = board_with_cards();
        open_card_search(&mut app);
        for c in "@ana #DOING".chars() {
            search_input_push(&mut app, c);
        }
        assert_eq!(app.view_cards(10).len(), 2);
        app.search_input.clear();
        for c in "@nobody".chars() {
            search_input_push(&mut app, c);
        }
        assert!(app.view_cards(10).is_empty());
    }
}
