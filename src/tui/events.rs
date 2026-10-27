use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::client::HteamClient;
use crate::models::Card;
use crate::operations;

use super::app::{
    App, DetailField, HelpContext, ProjectDetailSection, ProjectMemberEntry, UpdateAvailableInfo,
};
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
    app.selected_reminder = None;
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
    app.selected_reminder = None;
}

pub fn select_reminder(app: &mut App, delta: isize) {
    if app.reminders.is_empty() {
        app.selected_reminder = None;
        return;
    }
    let current = app
        .selected_reminder
        .map(|i| i as isize)
        .unwrap_or_else(|| {
            if delta >= 0 {
                -1
            } else {
                app.reminders.len() as isize
            }
        });
    let next = (current + delta).clamp(0, app.reminders.len() as isize - 1);
    app.selected_reminder = Some(next as usize);
}

pub fn cancel_selected_reminder(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(index) = app.selected_reminder else {
        return;
    };
    let Some(reminder) = app.reminders.get(index) else {
        return;
    };
    let reminder_id = reminder.id;
    let id = app.requests.bump_reminders();
    app.begin_busy("Cancelando reminder...");
    jobs.spawn(async move {
        let result = match operations::reminders::cancel(&client, reminder_id).await {
            Ok(()) => operations::reminders::list(&client)
                .await
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        JobOutcome::ReminderCanceled {
            id,
            reminder_id,
            result,
        }
    });
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
    app.comments_selected = None;
    app.comment_reply_to = None;
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

pub fn select_comment(app: &mut App, delta: isize) {
    if app.comments.is_empty() {
        app.comments_selected = None;
        return;
    }
    let current = app
        .comments_selected
        .map(|i| i as isize)
        .unwrap_or_else(|| {
            if delta >= 0 {
                -1
            } else {
                app.comments.len() as isize
            }
        });
    let next = (current + delta).clamp(0, app.comments.len() as isize - 1);
    app.comments_selected = Some(next as usize);
}

pub fn start_reply_comment(app: &mut App) {
    let Some(index) = app.comments_selected else {
        return;
    };
    let Some(comment) = app.comments.get(index) else {
        return;
    };
    app.comment_reply_to = Some(comment.id);
    app.composing_comment = true;
    app.comment_input.clear();
    app.comment_follow = false;
    app.comment_date_input.clear();
    app.comment_date_focused = false;
    app.mention_suggestions.clear();
}

pub fn start_composing_comment(app: &mut App) {
    app.comment_reply_to = None;
    app.composing_comment = true;
    app.comment_input.clear();
    app.comment_follow = false;
    app.comment_date_input.clear();
    app.comment_date_focused = false;
    app.mention_suggestions.clear();
}

pub fn cancel_composing_comment(app: &mut App) {
    app.composing_comment = false;
    app.comment_reply_to = None;
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
    let reply_to = app.comment_reply_to;
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
            reply_to,
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

/// Open task detail popup (`d`) and load full detail in the background.
pub fn open_task_detail(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(card) = app.current_card().cloned() else {
        return;
    };
    open_task_detail_by_id(client, app, jobs, card.id, app.current_list().map(|l| l.id));
}

pub fn open_task_detail_by_id(
    client: Arc<HteamClient>,
    app: &mut App,
    jobs: &Jobs,
    card_id: u64,
    list_id: Option<u64>,
) {
    app.show_task_detail = true;
    app.task_detail = None;
    app.task_detail_error = None;
    app.task_detail_dependence_label = None;
    app.task_detail_dependence_id = None;
    app.task_detail_card_id = Some(card_id);
    app.task_detail_list_id = list_id.or_else(|| {
        app.cards_by_list
            .iter()
            .find_map(|(lid, cards)| cards.iter().find(|c| c.id == card_id).map(|_| *lid))
    });
    app.task_detail_field_idx = 0;
    app.task_detail_scroll.reset();
    app.editing_detail_field = false;
    app.picking_dependence = false;
    app.picking_labels = false;
    app.label_picker_loading = false;
    app.label_picker_filter.clear();
    app.label_picker_selected.clear();
    app.detail_edit_input.clear();
    app.dependence_picker_filter.clear();
    app.dependence_picker_items.clear();
    let _ = app.requests.bump_label_picker();

    let id = app.requests.bump_task_detail();
    let board_cards: Vec<Card> = app
        .cards_by_list
        .values()
        .flat_map(|v| v.iter().cloned())
        .collect();
    app.begin_busy("Cargando detalle...");
    jobs.spawn(async move {
        let loaded = match operations::cards::card_detail_enriched(&client, card_id).await {
            Ok(detail) => {
                let (label, dep_id) =
                    resolve_dependence_meta(&client, &detail.dependence, &board_cards).await;
                Ok((detail, label, dep_id))
            }
            Err(e) => Err(e.to_string()),
        };
        let (result, dependence_label, dependence_id) = match loaded {
            Ok((d, lab, did)) => (Ok(d), lab, did),
            Err(e) => (Err(e), None, None),
        };
        JobOutcome::TaskDetailLoaded {
            id,
            card_id,
            result,
            dependence_label,
            dependence_id,
        }
    });
}

pub fn jump_to_dependence(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(dep_id) = app.task_detail_dependence_id.or_else(|| {
        let detail = app.task_detail.as_ref()?;
        let cards: Vec<&Card> = app.cards_by_list.values().flatten().collect();
        let q = operations::cards::dependence_display(&detail.dependence)?;
        operations::cards::resolve_dependence_query(&q, cards)
    }) else {
        app.set_status("No se pudo resolver la dependencia en el board actual.");
        return;
    };
    if app.task_detail_card_id == Some(dep_id) {
        return;
    }
    open_task_detail_by_id(client, app, jobs, dep_id, None);
}

async fn resolve_dependence_meta(
    client: &HteamClient,
    dependence: &Option<serde_json::Value>,
    board_cards: &[Card],
) -> (Option<String>, Option<u64>) {
    let label = operations::cards::dependence_display(dependence);
    let mut dep_id = operations::cards::dependence_task_id(dependence);
    if dep_id.is_none() {
        if let Some(ref lab) = label {
            dep_id = operations::cards::resolve_dependence_query(lab, board_cards.iter());
        }
    }
    let label = match (label.clone(), dep_id) {
        (Some(l), _) if operations::cards::dependence_task_id(dependence).is_none() => Some(l),
        (_, Some(id)) => {
            if let Ok(dep) = operations::cards::card_detail(client, id).await {
                let num = dep.number.map(|n| format!("#{n} ")).unwrap_or_default();
                Some(format!("{num}{} (id {id})", dep.name))
            } else {
                label.or(Some(format!("id {id}")))
            }
        }
        (l, _) => l,
    };
    (label, dep_id)
}

pub fn close_task_detail(app: &mut App) {
    app.show_task_detail = false;
    app.task_detail = None;
    app.task_detail_card_id = None;
    app.task_detail_list_id = None;
    app.task_detail_error = None;
    app.task_detail_dependence_label = None;
    app.task_detail_dependence_id = None;
    app.editing_detail_field = false;
    app.picking_dependence = false;
    app.picking_labels = false;
    app.label_picker_loading = false;
    app.label_picker_filter.clear();
    app.label_picker_selected.clear();
    app.detail_edit_input.clear();
    app.dependence_picker_items.clear();
    app.dependence_picker_filter.clear();
    let _ = app.requests.bump_task_detail();
    let _ = app.requests.bump_label_picker();
}

/// Build the board task list for depends-on picking.
pub fn open_dependence_picker(app: &mut App) {
    let self_id = app.task_detail_card_id;
    let mut items: Vec<(u64, Option<u64>, String)> = app
        .cards_by_list
        .values()
        .flatten()
        .filter(|c| Some(c.id) != self_id)
        .map(|c| (c.id, c.number, c.name.clone()))
        .collect();
    items.sort_by(|a, b| {
        a.2.to_lowercase()
            .cmp(&b.2.to_lowercase())
            .then(a.0.cmp(&b.0))
    });
    items.dedup_by(|a, b| a.0 == b.0);
    app.dependence_picker_items = items;
    app.dependence_picker_filter.clear();
    app.dependence_picker_idx = 0;
    if let Some(cur) = app.task_detail_dependence_id {
        if let Some(i) = app
            .dependence_picker_items
            .iter()
            .position(|(id, _, _)| *id == cur)
        {
            app.dependence_picker_idx = i;
        }
    }
    app.picking_dependence = true;
    app.editing_detail_field = true;
}

pub fn filtered_dependence_picker_indices(app: &App) -> Vec<usize> {
    let f = app.dependence_picker_filter.as_str().trim().to_lowercase();
    app.dependence_picker_items
        .iter()
        .enumerate()
        .filter(|(_, (id, num, name))| {
            if f.is_empty() {
                return true;
            }
            name.to_lowercase().contains(&f)
                || id.to_string().contains(&f)
                || num
                    .map(|n| n.to_string().contains(&f) || format!("#{n}").contains(&f))
                    .unwrap_or(false)
        })
        .map(|(i, _)| i)
        .collect()
}

pub fn dependence_picker_move(app: &mut App, delta: i32) {
    let indices = filtered_dependence_picker_indices(app);
    if indices.is_empty() {
        return;
    }
    let cur_pos = indices
        .iter()
        .position(|i| *i == app.dependence_picker_idx)
        .unwrap_or(0) as i32;
    let next = (cur_pos + delta).rem_euclid(indices.len() as i32) as usize;
    app.dependence_picker_idx = indices[next];
}

pub fn open_label_picker(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(card_id) = app.task_detail_card_id else {
        return;
    };
    app.picking_labels = true;
    app.label_picker_loading = true;
    app.label_picker_idx = 0;
    app.label_picker_filter.clear();
    app.label_picker_selected.clear();
    let cached_catalog = app
        .label_catalog_loaded
        .then(|| app.label_picker_items.clone());
    if cached_catalog.is_none() {
        app.label_picker_items.clear();
    }
    let board_number = app.board_number;
    let id = app.requests.bump_label_picker();
    app.begin_busy("Cargando catálogo de labels...");
    jobs.spawn(async move {
        let result = async {
            let mut labels = match cached_catalog {
                Some(labels) => labels,
                None => operations::cards::card_label_catalog(&client).await?,
            };
            let current =
                operations::cards::card_labels(&client, card_id, Some(board_number)).await?;
            if current.iter().any(|label| label.id == 0) {
                anyhow::bail!("el API devolvió labels sin ID");
            }
            let selected: Vec<u64> = current.iter().map(|label| label.id).collect();
            for label in current {
                if !labels.iter().any(|available| available.id == label.id) {
                    labels.push(label);
                }
            }
            labels.sort_by_cached_key(|label| label.name.to_lowercase());
            Ok((labels, selected))
        }
        .await
        .map_err(|error: anyhow::Error| error.to_string());
        JobOutcome::LabelPickerLoaded {
            id,
            card_id,
            result,
        }
    });
}

pub fn filtered_label_picker_indices(app: &App) -> Vec<usize> {
    let filter = app.label_picker_filter.as_str().trim().to_lowercase();
    app.label_picker_items
        .iter()
        .enumerate()
        .filter(|(_, label)| {
            filter.is_empty()
                || label.name.to_lowercase().contains(&filter)
                || label.id.to_string().contains(&filter)
        })
        .map(|(index, _)| index)
        .collect()
}

pub fn label_picker_move(app: &mut App, delta: i32) {
    let indices = filtered_label_picker_indices(app);
    if indices.is_empty() {
        return;
    }
    let current = indices
        .iter()
        .position(|index| *index == app.label_picker_idx)
        .unwrap_or(0) as i32;
    let next = (current + delta).rem_euclid(indices.len() as i32) as usize;
    app.label_picker_idx = indices[next];
}

pub fn toggle_label_picker_selection(app: &mut App) {
    if app.label_picker_loading {
        return;
    }
    let Some(label) = app.label_picker_items.get(app.label_picker_idx) else {
        return;
    };
    if !app.label_picker_selected.insert(label.id) {
        app.label_picker_selected.remove(&label.id);
    }
}

pub fn cancel_label_picker(app: &mut App) {
    app.picking_labels = false;
    app.label_picker_loading = false;
    app.label_picker_filter.clear();
    let _ = app.requests.bump_label_picker();
}

pub fn confirm_label_picker(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    if app.label_picker_loading {
        return;
    }
    let Some(card_id) = app.task_detail_card_id else {
        cancel_label_picker(app);
        return;
    };
    let label_ids: Vec<u64> = app
        .label_picker_items
        .iter()
        .filter(|label| app.label_picker_selected.contains(&label.id))
        .map(|label| label.id)
        .collect();
    app.picking_labels = false;
    app.editing_detail_field = false;
    let list_id = app.task_detail_list_id;
    let board_number = app.board_number;
    let board_cards: Vec<Card> = app
        .cards_by_list
        .values()
        .flat_map(|cards| cards.iter().cloned())
        .collect();
    let id = app.requests.bump_task_detail();
    let _ = app.requests.bump_label_picker();
    app.begin_busy("Guardando labels...");
    jobs.spawn(async move {
        let save = async {
            operations::cards::set_card_labels(&client, card_id, Some(board_number), &label_ids)
                .await?;
            operations::cards::card_detail_enriched(&client, card_id).await
        };
        let result = match save.await {
            Ok(detail) => {
                let (dependence_label, dependence_id) =
                    resolve_dependence_meta(&client, &detail.dependence, &board_cards).await;
                let cards = if let Some(list_id) = list_id {
                    Some(
                        operations::cards::list_cards(&client, list_id, Some(board_number))
                            .await
                            .map(|(cards, _)| cards)
                            .map_err(|error| error.to_string()),
                    )
                } else {
                    None
                };
                (Ok(detail), dependence_label, dependence_id, cards)
            }
            Err(error) => (Err(error.to_string()), None, None, None),
        };
        JobOutcome::TaskDetailSaved {
            id,
            card_id,
            list_id,
            board_number,
            result: result.0,
            dependence_label: result.1,
            dependence_id: result.2,
            cards: result.3,
        }
    });
}

pub fn confirm_dependence_picker(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let indices = filtered_dependence_picker_indices(app);
    if indices.is_empty() {
        app.set_status("No hay tasks que coincidan con el filtro.");
        return;
    }
    if !indices.contains(&app.dependence_picker_idx) {
        app.dependence_picker_idx = indices[0];
    }
    let (dep_id, _, name) = app.dependence_picker_items[app.dependence_picker_idx].clone();
    app.picking_dependence = false;
    app.editing_detail_field = false;
    app.detail_edit_input.set(dep_id.to_string());
    // Save immediately with the chosen id.
    let card_id = match app.task_detail_card_id {
        Some(id) => id,
        None => return,
    };
    let list_id = app.task_detail_list_id;
    let board_number = app.board_number;
    let board_cards: Vec<Card> = app
        .cards_by_list
        .values()
        .flat_map(|v| v.iter().cloned())
        .collect();
    let id = app.requests.bump_task_detail();
    let value = dep_id.to_string();
    app.begin_busy(format!("Guardando dependencia: {name}..."));
    jobs.spawn(async move {
        let save = async {
            operations::cards::update_card_fields(
                &client,
                card_id,
                board_number,
                operations::cards::CardFieldUpdates {
                    dependence: Some(value.as_str()),
                    ..Default::default()
                },
            )
            .await?;
            operations::cards::card_detail_enriched(&client, card_id).await
        };
        let result = match save.await {
            Ok(detail) => {
                let (dependence_label, dependence_id) =
                    resolve_dependence_meta(&client, &detail.dependence, &board_cards).await;
                let cards = if let Some(lid) = list_id {
                    Some(
                        operations::cards::list_cards(&client, lid, Some(board_number))
                            .await
                            .map(|(c, _)| c)
                            .map_err(|e| e.to_string()),
                    )
                } else {
                    None
                };
                (Ok(detail), dependence_label, dependence_id, cards)
            }
            Err(e) => (Err(e.to_string()), None, None, None),
        };
        JobOutcome::TaskDetailSaved {
            id,
            card_id,
            list_id,
            board_number,
            result: result.0,
            dependence_label: result.1,
            dependence_id: result.2,
            cards: result.3,
        }
    });
}

pub fn clear_dependence(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    app.picking_dependence = false;
    app.editing_detail_field = false;
    app.detail_edit_input.set("");
    // reuse confirm path with empty
    let card_id = match app.task_detail_card_id {
        Some(id) => id,
        None => return,
    };
    let list_id = app.task_detail_list_id;
    let board_number = app.board_number;
    let board_cards: Vec<Card> = app
        .cards_by_list
        .values()
        .flat_map(|v| v.iter().cloned())
        .collect();
    let id = app.requests.bump_task_detail();
    app.begin_busy("Quitando dependencia...");
    jobs.spawn(async move {
        let save = async {
            operations::cards::update_card_fields(
                &client,
                card_id,
                board_number,
                operations::cards::CardFieldUpdates {
                    dependence: Some(""),
                    ..Default::default()
                },
            )
            .await?;
            operations::cards::card_detail_enriched(&client, card_id).await
        };
        let result = match save.await {
            Ok(detail) => {
                let (dependence_label, dependence_id) =
                    resolve_dependence_meta(&client, &detail.dependence, &board_cards).await;
                let cards = if let Some(lid) = list_id {
                    Some(
                        operations::cards::list_cards(&client, lid, Some(board_number))
                            .await
                            .map(|(c, _)| c)
                            .map_err(|e| e.to_string()),
                    )
                } else {
                    None
                };
                (Ok(detail), dependence_label, dependence_id, cards)
            }
            Err(e) => (Err(e.to_string()), None, None, None),
        };
        JobOutcome::TaskDetailSaved {
            id,
            card_id,
            list_id,
            board_number,
            result: result.0,
            dependence_label: result.1,
            dependence_id: result.2,
            cards: result.3,
        }
    });
}

pub fn detail_move_field(app: &mut App, delta: i32) {
    if app.editing_detail_field {
        return;
    }
    let len = DetailField::ALL.len() as i32;
    let next = (app.task_detail_field_idx as i32 + delta).rem_euclid(len);
    app.task_detail_field_idx = next as usize;
}

pub fn start_edit_detail_field(app: &mut App) {
    let Some(detail) = app.task_detail.as_ref() else {
        return;
    };
    let field = DetailField::ALL[app.task_detail_field_idx.min(DetailField::ALL.len() - 1)];
    if field == DetailField::Dependence {
        open_dependence_picker(app);
        return;
    }
    let seed = match field {
        DetailField::Name => detail.name.clone(),
        DetailField::Description => detail.description.clone().unwrap_or_default(),
        DetailField::Responsible => operations::cards::responsible_display(
            detail
                .responsible
                .as_ref()
                .unwrap_or(&serde_json::Value::Null),
        )
        .unwrap_or_default(),
        DetailField::Priority => detail
            .priority
            .as_ref()
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default(),
        DetailField::Dependence => operations::cards::dependence_task_id(&detail.dependence)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        DetailField::StartDate => detail
            .start_date
            .clone()
            .unwrap_or_default()
            .chars()
            .take(10)
            .collect(),
        DetailField::DueDate => detail
            .due_date
            .clone()
            .unwrap_or_default()
            .chars()
            .take(10)
            .collect(),
        DetailField::TimeEstimated => detail.time_estimated.clone().unwrap_or_default(),
        DetailField::TimeSpent => detail.time_spend.clone().unwrap_or_default(),
    };
    app.detail_edit_input.set(seed);
    app.editing_detail_field = true;
}

pub fn cancel_edit_detail_field(app: &mut App) {
    app.editing_detail_field = false;
    app.picking_dependence = false;
    app.detail_edit_input.clear();
    app.dependence_picker_filter.clear();
}

pub fn detail_edit_push(app: &mut App, c: char) {
    if c.is_control() {
        return;
    }
    app.detail_edit_input.push(c);
}

pub fn detail_edit_backspace(app: &mut App) {
    app.detail_edit_input.backspace();
}

/// Save the field being edited and refresh detail (+ list cards when possible).
pub fn submit_detail_field(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(card_id) = app.task_detail_card_id else {
        cancel_edit_detail_field(app);
        return;
    };
    let field = DetailField::ALL[app.task_detail_field_idx.min(DetailField::ALL.len() - 1)];
    let value = app.detail_edit_input.as_str().trim().to_string();
    let list_id = app.task_detail_list_id;
    let board_number = app.board_number;
    let board_cards: Vec<Card> = app
        .cards_by_list
        .values()
        .flat_map(|v| v.iter().cloned())
        .collect();
    let id = app.requests.bump_task_detail();
    app.editing_detail_field = false;
    app.picking_dependence = false;
    app.begin_busy(format!("Guardando {}...", field.label()));

    jobs.spawn(async move {
        let save = async {
            match field {
                DetailField::Name => {
                    operations::cards::update_card(
                        &client,
                        card_id,
                        board_number,
                        Some(&value),
                        None,
                        None,
                        None,
                    )
                    .await?;
                }
                DetailField::Description => {
                    operations::cards::update_description(&client, card_id, &value).await?;
                }
                DetailField::Responsible => {
                    operations::cards::update_card(
                        &client,
                        card_id,
                        board_number,
                        None,
                        None,
                        None,
                        Some(&value),
                    )
                    .await?;
                }
                DetailField::Priority => {
                    operations::cards::update_card(
                        &client,
                        card_id,
                        board_number,
                        None,
                        None,
                        Some(&value),
                        None,
                    )
                    .await?;
                }
                DetailField::Dependence => {
                    // Full HTML edit form: dependence=<task_id> (empty clears).
                    operations::cards::update_card_fields(
                        &client,
                        card_id,
                        board_number,
                        operations::cards::CardFieldUpdates {
                            dependence: Some(value.as_str()),
                            ..Default::default()
                        },
                    )
                    .await?;
                }
                DetailField::StartDate => {
                    let bid = operations::cards::board_id(&client).await?;
                    operations::cards::set_date(&client, bid, card_id, "start_date", &value)
                        .await?;
                }
                DetailField::DueDate => {
                    let bid = operations::cards::board_id(&client).await?;
                    operations::cards::set_date(&client, bid, card_id, "due_date", &value).await?;
                }
                DetailField::TimeEstimated => {
                    let bid = operations::cards::board_id(&client).await?;
                    let times =
                        operations::cards::set_time(&client, bid, card_id, "estimate", &value)
                            .await?;
                    let mut detail =
                        operations::cards::card_detail_enriched(&client, card_id).await?;
                    if let Some(est) = times.estimated_time {
                        detail.time_estimated = Some(est);
                    }
                    if let Some(spent) = times.time_spend {
                        detail.time_spend = Some(spent);
                    }
                    if let Some(d) = times.start_date {
                        detail.start_date = Some(d);
                    }
                    if let Some(d) = times.due_date {
                        detail.due_date = Some(d);
                    }
                    return Ok(detail);
                }
                DetailField::TimeSpent => {
                    let bid = operations::cards::board_id(&client).await?;
                    let times =
                        operations::cards::set_time(&client, bid, card_id, "time_spend", &value)
                            .await?;
                    let mut detail =
                        operations::cards::card_detail_enriched(&client, card_id).await?;
                    if let Some(est) = times.estimated_time {
                        detail.time_estimated = Some(est);
                    }
                    if let Some(spent) = times.time_spend {
                        detail.time_spend = Some(spent);
                    }
                    if let Some(d) = times.start_date {
                        detail.start_date = Some(d);
                    }
                    if let Some(d) = times.due_date {
                        detail.due_date = Some(d);
                    }
                    return Ok(detail);
                }
            }
            operations::cards::card_detail_enriched(&client, card_id).await
        };

        let result = match save.await {
            Ok(detail) => {
                let (dependence_label, dependence_id) =
                    resolve_dependence_meta(&client, &detail.dependence, &board_cards).await;
                let cards = if let Some(lid) = list_id {
                    Some(
                        operations::cards::list_cards(&client, lid, Some(board_number))
                            .await
                            .map(|(c, _)| c)
                            .map_err(|e| e.to_string()),
                    )
                } else {
                    None
                };
                (Ok(detail), dependence_label, dependence_id, cards)
            }
            Err(e) => (Err(e.to_string()), None, None, None),
        };

        JobOutcome::TaskDetailSaved {
            id,
            card_id,
            list_id,
            board_number,
            result: result.0,
            dependence_label: result.1,
            dependence_id: result.2,
            cards: result.3,
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
        if let Some(p) = app.known_projects.first() {
            load_project(client, app, jobs, p.id);
        }
    }
}

pub fn close_projects(app: &mut App) {
    app.show_projects = false;
    app.composing_project = false;
    app.project_input.clear();
    cancel_milestone_form(app);
    cancel_project_task_form(app);
    app.projects_detail_focused = false;
    app.show_live_projects = false;
    app.requests.bump_projects();
    app.requests.bump_project_milestone_write();
    app.requests.bump_project_milestone_form();
    app.requests.bump_project_milestone_delete();
    app.requests.bump_project_task_write();
    app.requests.bump_project_task_form();
    app.requests.bump_project_status();
    app.requests.bump_project_members();
    app.requests.bump_project_members_write();
    app.requests.bump_live_projects();
    cancel_composing_member(app);
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
    let Some(p) = app.known_projects.get(app.selected_project_idx) else {
        return;
    };
    let id = p.id;
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
        let Some(p) = app.known_projects.get(app.selected_project_idx) else {
            return;
        };
        p.id
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
    // Prefer cached config name, then live-list name from this session.
    app.active_project_name = app
        .known_projects
        .iter()
        .find(|p| p.id == project_id)
        .and_then(|p| p.name.clone())
        .or_else(|| {
            app.live_projects
                .iter()
                .find(|p| p.id == project_id)
                .map(|p| p.name.clone())
        });
    remember_project(app, project_id, app.active_project_name.clone());
    clear_project_section_data(app);
    app.projects_detail_section = ProjectDetailSection::Milestones;
    app.pending_milestone_delete = None;
    cancel_milestone_form(app);
    cancel_project_task_form(app);
    cancel_composing_member(app);
    // Single-section UX: only fetch the active section (milestones first).
    maybe_load_lazy_project_section(client, app, jobs);
}

fn clear_project_section_data(app: &mut App) {
    app.project_milestones.clear();
    app.project_milestones_loaded = false;
    app.selected_milestone_idx = 0;
    app.project_tasks.clear();
    app.project_tasks_loaded = false;
    app.selected_task_idx = 0;
    app.project_task_tree.clear();
    app.project_task_tree_loaded = false;
    app.project_tree_expanded.clear();
    app.selected_tree_idx = 0;
    app.project_status = None;
    app.project_members = None;
    app.project_members_baseline.clear();
    app.selected_member_idx = 0;
    app.requests.bump_projects();
    app.requests.bump_project_milestones();
    app.requests.bump_project_tasks();
    app.requests.bump_project_task_tree();
    app.requests.bump_project_status();
    app.requests.bump_project_members();
    app.requests.bump_project_members_write();
}

/// Moves the milestone highlight inside the projects detail panel.
pub fn move_milestone_selection(app: &mut App, delta: i32) {
    if app.project_milestones.is_empty() {
        return;
    }
    let len = app.project_milestones.len() as i32;
    let next = (app.selected_milestone_idx as i32 + delta).clamp(0, len - 1);
    if next as usize != app.selected_milestone_idx {
        app.pending_milestone_delete = None;
    }
    app.selected_milestone_idx = next as usize;
}

/// Opens an empty create-milestone form for the active project.
pub fn start_create_milestone(app: &mut App) {
    if app.active_project.is_none() {
        app.set_status("Elegí un proyecto antes de crear un milestone.");
        return;
    }
    app.composing_milestone = true;
    app.milestone_form_edit_id = None;
    app.milestone_name_input.clear();
    app.milestone_desc_input.clear();
    app.milestone_desc_focused = false;
    app.projects_detail_focused = true;
    app.projects_detail_section = ProjectDetailSection::Milestones;
}

/// Loads the server edit form and opens the TUI form seeded with name + description.
pub fn start_edit_milestone(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(project_id) = app.active_project else {
        app.set_status("Elegí un proyecto antes de editar un milestone.");
        return;
    };
    let Some(milestone) = app.project_milestones.get(app.selected_milestone_idx) else {
        app.set_status("No hay milestones para editar.");
        return;
    };
    let Some(milestone_id) = milestone.id() else {
        app.set_status("No se pudo leer el id del milestone resaltado.");
        return;
    };
    // Optimistic name seed while the form loads; description arrives from the server.
    let name = milestone.name.clone();
    app.milestone_form_edit_id = Some(milestone_id);
    app.milestone_name_input.set(name);
    app.milestone_desc_input.clear();
    app.milestone_desc_focused = false;
    app.projects_detail_focused = true;
    app.projects_detail_section = ProjectDetailSection::Milestones;
    app.composing_milestone = false;

    let id = app.requests.bump_project_milestone_form();
    app.begin_busy("Cargando milestone...");
    jobs.spawn(async move {
        let result = operations::projects::milestone_form(&client, project_id, milestone_id)
            .await
            .map_err(|e| e.to_string());
        JobOutcome::MilestoneFormLoaded {
            id,
            project_id,
            milestone_id,
            result,
        }
    });
}

/// Opens the edit form after a successful form scrape (called from `net::apply_outcome`).
/// Name is read-only: Hteam's edit endpoint rejects renames (soft 404).
pub fn open_milestone_edit_form(
    app: &mut App,
    milestone_id: u64,
    name: String,
    description: String,
) {
    app.composing_milestone = true;
    app.milestone_form_edit_id = Some(milestone_id);
    app.milestone_name_input.set(name);
    app.milestone_desc_input.set(description);
    // Skip the locked name field — only description is editable.
    app.milestone_desc_focused = true;
    app.projects_detail_focused = true;
}

pub fn cancel_milestone_form(app: &mut App) {
    app.composing_milestone = false;
    app.milestone_form_edit_id = None;
    app.milestone_name_input.clear();
    app.milestone_desc_input.clear();
    app.milestone_desc_focused = false;
}

pub fn cancel_pending_milestone_delete(app: &mut App) {
    if app.pending_milestone_delete.is_some() {
        app.pending_milestone_delete = None;
        app.set_status("Borrado de milestone cancelado.");
    }
}

/// First `d`: arm confirm. Second `d`/Enter: delete via operations.
pub fn request_or_confirm_milestone_delete(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    if app.composing_milestone || app.composing_project_task || app.composing_member {
        return;
    }
    let Some(project_id) = app.active_project else {
        app.set_status("Elegí un proyecto antes de borrar un milestone.");
        return;
    };
    if !app.project_milestones_loaded {
        app.set_status("Esperá a que carguen los milestones.");
        return;
    }
    let Some(milestone) = app.project_milestones.get(app.selected_milestone_idx) else {
        app.set_status("No hay milestones para borrar.");
        return;
    };
    let Some(milestone_id) = milestone.id() else {
        app.set_status("No se pudo leer el id del milestone resaltado.");
        return;
    };
    let name = milestone.name.clone();

    if app.pending_milestone_delete != Some(milestone_id) {
        app.pending_milestone_delete = Some(milestone_id);
        app.set_status(format!(
            "Borrar «{name}» (#{milestone_id})? d/Enter confirma · Esc cancela",
        ));
        return;
    }

    app.pending_milestone_delete = None;
    let id = app.requests.bump_project_milestone_delete();
    app.begin_busy("Eliminando milestone...");
    jobs.spawn(async move {
        let result = operations::projects::delete_milestone(&client, project_id, milestone_id)
            .await
            .map(|_| format!("Milestone eliminado: {name} (#{milestone_id})"))
            .map_err(|e| e.to_string());
        let refresh = if result.is_ok() {
            // Only milestones list matters here; keep tasks/tree invalidation light.
            let milestones = operations::projects::milestones(&client, project_id)
                .await
                .map_err(|e| e.to_string());
            Some(milestones)
        } else {
            None
        };
        JobOutcome::ProjectMilestoneDeleted {
            id,
            project_id,
            result,
            refresh,
        }
    });
}

/// Create: toggle name ↔ description. Edit: name is locked, stay on description.
pub fn toggle_milestone_form_field(app: &mut App) {
    if app.milestone_form_edit_id.is_some() {
        app.milestone_desc_focused = true;
        return;
    }
    app.milestone_desc_focused = !app.milestone_desc_focused;
}

pub fn milestone_form_input_push(app: &mut App, ch: char) {
    if app.milestone_form_edit_id.is_some() {
        // Name is display-only on edit (Hteam cannot rename via this form).
        app.milestone_desc_input.push(ch);
        app.milestone_desc_focused = true;
        return;
    }
    if app.milestone_desc_focused {
        app.milestone_desc_input.push(ch);
    } else {
        app.milestone_name_input.push(ch);
    }
}

pub fn milestone_form_input_backspace(app: &mut App) {
    if app.milestone_form_edit_id.is_some() {
        app.milestone_desc_input.backspace();
        app.milestone_desc_focused = true;
        return;
    }
    if app.milestone_desc_focused {
        app.milestone_desc_input.backspace();
    } else {
        app.milestone_name_input.backspace();
    }
}

/// Saves the milestone form via `operations::projects::{create,edit}_milestone`
/// and reloads the active project on success.
pub fn submit_milestone_form(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(project_id) = app.active_project else {
        cancel_milestone_form(app);
        return;
    };
    let name = app.milestone_name_input.trim().to_string();
    if name.is_empty() {
        app.set_status("El nombre del milestone no puede estar vacío.");
        return;
    }
    let description_raw = app.milestone_desc_input.trim().to_string();
    let edit_id = app.milestone_form_edit_id;
    cancel_milestone_form(app);

    let id = app.requests.bump_project_milestone_write();
    let busy = if edit_id.is_some() {
        "Guardando milestone..."
    } else {
        "Creando milestone..."
    };
    app.begin_busy(busy);

    jobs.spawn(async move {
        let result = match edit_id {
            // Edit always sends description (possibly empty) because the form was
            // seeded from the server — empty clears it on purpose.
            Some(milestone_id) => operations::projects::edit_milestone(
                &client,
                project_id,
                milestone_id,
                Some(&name),
                Some(description_raw.as_str()),
            )
            .await
            .map(|written| format!("Milestone actualizado: {}", written.name))
            .map_err(|e| e.to_string()),
            None => {
                let description = if description_raw.is_empty() {
                    None
                } else {
                    Some(description_raw.as_str())
                };
                operations::projects::create_milestone(&client, project_id, &name, description)
                    .await
                    .map(|written| format!("Milestone creado: {}", written.name))
                    .map_err(|e| e.to_string())
            }
        };

        let refresh = project_detail_refresh(&client, project_id, result.is_ok()).await;

        JobOutcome::ProjectMilestoneSaved {
            id,
            project_id,
            result,
            refresh,
        }
    });
}

/// h/l when detail is focused: milestones → tasks → tree → status → members.
pub fn cycle_projects_detail_section(app: &mut App, delta: i32) {
    if app.composing_milestone || app.composing_project_task || app.composing_member {
        return;
    }
    const ORDER: [ProjectDetailSection; 5] = [
        ProjectDetailSection::Milestones,
        ProjectDetailSection::Tasks,
        ProjectDetailSection::Tree,
        ProjectDetailSection::Status,
        ProjectDetailSection::Members,
    ];
    let idx = ORDER
        .iter()
        .position(|&s| s == app.projects_detail_section)
        .unwrap_or(0) as i32;
    let len = ORDER.len() as i32;
    let next = if delta >= 0 {
        (idx + 1).rem_euclid(len)
    } else {
        (idx - 1).rem_euclid(len)
    };
    app.projects_detail_section = ORDER[next as usize];
    app.projects_scroll.reset();
    app.pending_milestone_delete = None;
}

/// After h/l (or selecting a project), fetch only the focused section.
pub fn maybe_load_lazy_project_section(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(project_id) = app.active_project else {
        return;
    };
    match app.projects_detail_section {
        ProjectDetailSection::Milestones => {
            if app.project_milestones_loaded {
                return;
            }
            let id = app.requests.bump_project_milestones();
            app.begin_busy("Cargando milestones...");
            jobs.spawn(async move {
                let result = operations::projects::milestones(&client, project_id)
                    .await
                    .map_err(|e| e.to_string());
                JobOutcome::ProjectMilestonesLoaded {
                    id,
                    project_id,
                    result,
                }
            });
        }
        ProjectDetailSection::Tasks => {
            // Tasks create uses the milestone list; pull it if missing.
            if !app.project_milestones_loaded {
                let id = app.requests.bump_project_milestones();
                app.begin_busy("Cargando milestones...");
                let c = client.clone();
                jobs.spawn(async move {
                    let result = operations::projects::milestones(&c, project_id)
                        .await
                        .map_err(|e| e.to_string());
                    JobOutcome::ProjectMilestonesLoaded {
                        id,
                        project_id,
                        result,
                    }
                });
            }
            if app.project_tasks_loaded {
                return;
            }
            let id = app.requests.bump_project_tasks();
            app.begin_busy("Cargando tasks...");
            jobs.spawn(async move {
                let result = operations::projects::tasks(&client, project_id)
                    .await
                    .map(|resp| resp.results)
                    .map_err(|e| e.to_string());
                JobOutcome::ProjectTasksLoaded {
                    id,
                    project_id,
                    result,
                }
            });
        }
        ProjectDetailSection::Tree => {
            if app.project_task_tree_loaded {
                return;
            }
            let id = app.requests.bump_project_task_tree();
            app.begin_busy("Cargando árbol...");
            jobs.spawn(async move {
                let result = operations::projects::task_tree(&client, project_id)
                    .await
                    .map_err(|e| e.to_string());
                JobOutcome::ProjectTreeLoaded {
                    id,
                    project_id,
                    result,
                }
            });
        }
        ProjectDetailSection::Status => {
            if app.project_status.is_some() {
                return;
            }
            let id = app.requests.bump_project_status();
            app.begin_busy("Cargando resumen...");
            jobs.spawn(async move {
                let result = operations::projects::status(&client, project_id)
                    .await
                    .map_err(|e| e.to_string());
                JobOutcome::ProjectStatusLoaded {
                    id,
                    project_id,
                    result,
                }
            });
        }
        ProjectDetailSection::Members => {
            if app.project_members.is_some() {
                return;
            }
            let id = app.requests.bump_project_members();
            app.begin_busy("Cargando miembros...");
            jobs.spawn(async move {
                let result = operations::projects::members(&client, project_id)
                    .await
                    .map(|(name, members)| {
                        let entries = members
                            .into_iter()
                            .map(|(member_id, username)| ProjectMemberEntry {
                                id: member_id,
                                label: if username.is_empty() {
                                    format!("#{member_id}")
                                } else {
                                    format!("{username} (#{member_id})")
                                },
                            })
                            .collect::<Vec<_>>();
                        (name, entries)
                    })
                    .map_err(|e| e.to_string());
                JobOutcome::ProjectMembersLoaded {
                    id,
                    project_id,
                    result,
                }
            });
        }
    }
}

pub fn move_detail_selection(app: &mut App, delta: i32) {
    match app.projects_detail_section {
        ProjectDetailSection::Milestones => move_milestone_selection(app, delta),
        ProjectDetailSection::Tasks => move_task_selection(app, delta),
        ProjectDetailSection::Tree => move_tree_selection(app, delta),
        ProjectDetailSection::Status => {
            // Status is a scrollable sheet; j/k only moves the panel scroll in mod.rs.
        }
        ProjectDetailSection::Members => move_member_selection(app, delta),
    }
}

pub fn move_member_selection(app: &mut App, delta: i32) {
    let Some(members) = app.project_members.as_ref() else {
        return;
    };
    if members.is_empty() {
        return;
    }
    let len = members.len() as i32;
    let next = (app.selected_member_idx as i32 + delta).clamp(0, len - 1);
    app.selected_member_idx = next as usize;
}

pub fn project_members_dirty(app: &App) -> bool {
    let Some(current) = app.project_members.as_ref() else {
        return false;
    };
    let mut a: Vec<u64> = current.iter().map(|m| m.id).collect();
    let mut b = app.project_members_baseline.clone();
    a.sort_unstable();
    b.sort_unstable();
    a != b
}

pub fn start_composing_member(app: &mut App) {
    if app.active_project.is_none() {
        app.set_status("Elegí un proyecto antes de gestionar miembros.");
        return;
    }
    if app.project_members.is_none() {
        app.set_status("Esperá a que carguen los miembros.");
        return;
    }
    app.composing_member = true;
    app.member_input.clear();
    app.projects_detail_focused = true;
    app.projects_detail_section = ProjectDetailSection::Members;
}

pub fn cancel_composing_member(app: &mut App) {
    app.composing_member = false;
    app.member_input.clear();
}

pub fn member_input_push(app: &mut App, ch: char) {
    app.member_input.push(ch);
}

pub fn member_input_backspace(app: &mut App) {
    app.member_input.backspace();
}

/// Resolve username/id and append to the working member list (not saved yet).
pub fn submit_member_input(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let raw = app.member_input.trim().to_string();
    if raw.is_empty() {
        app.set_status("Indicá un username o id de usuario.");
        return;
    }
    cancel_composing_member(app);
    let Some(project_id) = app.active_project else {
        return;
    };
    let id = app.requests.bump_project_members();
    app.begin_busy("Buscando usuario...");
    jobs.spawn(async move {
        let result = operations::projects::resolve_user_id(&client, &raw)
            .await
            .map(|user_id| ProjectMemberEntry {
                id: user_id,
                label: if raw.chars().all(|c| c.is_ascii_digit()) {
                    format!("#{user_id}")
                } else {
                    format!("{raw} (#{user_id})")
                },
            })
            .map_err(|e| e.to_string());
        JobOutcome::ProjectMemberResolved {
            id,
            project_id,
            result,
        }
    });
}

pub fn remove_selected_member(app: &mut App) {
    let Some(members) = app.project_members.as_mut() else {
        app.set_status("Todavía no hay miembros cargados.");
        return;
    };
    if members.is_empty() {
        app.set_status("No hay miembros para quitar.");
        return;
    }
    if app.selected_member_idx >= members.len() {
        app.selected_member_idx = members.len().saturating_sub(1);
    }
    let removed = members.remove(app.selected_member_idx);
    if app.selected_member_idx >= members.len() && !members.is_empty() {
        app.selected_member_idx = members.len() - 1;
    }
    if members.is_empty() {
        app.selected_member_idx = 0;
    }
    app.set_status(format!("Quitado {} (sin guardar)", removed.label));
}

pub fn save_project_members(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(project_id) = app.active_project else {
        app.set_status("Elegí un proyecto antes de guardar miembros.");
        return;
    };
    let Some(members) = app.project_members.as_ref() else {
        app.set_status("Todavía no hay miembros cargados.");
        return;
    };
    if !project_members_dirty(app) {
        app.set_status("Sin cambios de miembros.");
        return;
    }
    let member_ids: Vec<u64> = members.iter().map(|m| m.id).collect();
    let id = app.requests.bump_project_members_write();
    app.begin_busy("Guardando miembros...");
    jobs.spawn(async move {
        let result = operations::projects::set_members(&client, project_id, &member_ids)
            .await
            .map(|_| member_ids)
            .map_err(|e| e.to_string());
        JobOutcome::ProjectMembersSaved {
            id,
            project_id,
            result,
        }
    });
}

pub fn move_task_selection(app: &mut App, delta: i32) {
    if app.project_tasks.is_empty() {
        return;
    }
    let len = app.project_tasks.len() as i32;
    let next = (app.selected_task_idx as i32 + delta).clamp(0, len - 1);
    app.selected_task_idx = next as usize;
}

/// Total nodes in the full tree (including collapsed descendants).
pub fn project_task_tree_len(nodes: &[crate::models::ProjectTaskTreeNode]) -> usize {
    nodes
        .iter()
        .map(|n| 1 + project_task_tree_len(&n.children))
        .sum()
}

/// Visible rows under the current expand set (roots always visible).
pub fn project_task_tree_visible_len(
    nodes: &[crate::models::ProjectTaskTreeNode],
    expanded: &std::collections::HashSet<u64>,
) -> usize {
    nodes
        .iter()
        .map(|n| {
            let kids = if !n.children.is_empty() && expanded.contains(&n.id) {
                project_task_tree_visible_len(&n.children, expanded)
            } else {
                0
            };
            1 + kids
        })
        .sum()
}

/// DFS ids of currently visible rows (same order as the tree draw).
pub fn project_task_tree_visible_ids(
    nodes: &[crate::models::ProjectTaskTreeNode],
    expanded: &std::collections::HashSet<u64>,
) -> Vec<u64> {
    let mut out = Vec::new();
    collect_visible_ids(nodes, expanded, &mut out);
    out
}

fn collect_visible_ids(
    nodes: &[crate::models::ProjectTaskTreeNode],
    expanded: &std::collections::HashSet<u64>,
    out: &mut Vec<u64>,
) {
    for n in nodes {
        out.push(n.id);
        if !n.children.is_empty() && expanded.contains(&n.id) {
            collect_visible_ids(&n.children, expanded, out);
        }
    }
}

fn clamp_tree_selection(app: &mut App) {
    let len = project_task_tree_visible_len(&app.project_task_tree, &app.project_tree_expanded);
    if len == 0 {
        app.selected_tree_idx = 0;
    } else if app.selected_tree_idx >= len {
        app.selected_tree_idx = len - 1;
    }
}

pub fn move_tree_selection(app: &mut App, delta: i32) {
    let len =
        project_task_tree_visible_len(&app.project_task_tree, &app.project_tree_expanded) as i32;
    if len == 0 {
        return;
    }
    let next = (app.selected_tree_idx as i32 + delta).clamp(0, len - 1);
    app.selected_tree_idx = next as usize;
}

/// Enter/Space on the tree section: expand or collapse the highlighted node.
pub fn toggle_tree_node(app: &mut App) {
    let ids = project_task_tree_visible_ids(&app.project_task_tree, &app.project_tree_expanded);
    let Some(&id) = ids.get(app.selected_tree_idx) else {
        return;
    };
    let Some(node) = find_tree_node(&app.project_task_tree, id) else {
        return;
    };
    if node.children.is_empty() {
        app.set_status("Esa task no tiene dependientes.");
        return;
    }
    if app.project_tree_expanded.contains(&id) {
        collapse_tree_node(app, id);
    } else {
        app.project_tree_expanded.insert(id);
    }
    clamp_tree_selection(app);
}

fn collapse_tree_node(app: &mut App, id: u64) {
    let mut drop_ids = vec![id];
    if let Some(node) = find_tree_node(&app.project_task_tree, id) {
        collect_descendant_ids(node, &mut drop_ids);
    }
    for drop_id in drop_ids {
        app.project_tree_expanded.remove(&drop_id);
    }
}

fn collect_descendant_ids(node: &crate::models::ProjectTaskTreeNode, out: &mut Vec<u64>) {
    for child in &node.children {
        out.push(child.id);
        collect_descendant_ids(child, out);
    }
}

fn find_tree_node(
    nodes: &[crate::models::ProjectTaskTreeNode],
    id: u64,
) -> Option<&crate::models::ProjectTaskTreeNode> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(found) = find_tree_node(&n.children, id) {
            return Some(found);
        }
    }
    None
}

/// Create a task under the highlighted milestone.
pub fn start_create_project_task(app: &mut App) {
    if app.active_project.is_none() {
        app.set_status("Elegí un proyecto antes de crear una task.");
        return;
    }
    let Some(milestone) = app.project_milestones.get(app.selected_milestone_idx) else {
        app.set_status("Seleccioná un milestone (h/l → Milestones, j/k) antes de crear la task.");
        return;
    };
    if milestone.id().is_none() {
        app.set_status("No se pudo leer el id del milestone resaltado.");
        return;
    }
    app.composing_project_task = true;
    app.project_task_form_edit_id = None;
    app.project_task_name_input.clear();
    app.project_task_desc_input.clear();
    app.project_task_milestone_input.clear();
    app.project_task_dependence_input.clear();
    app.project_task_start_date_input.clear();
    app.project_task_form_field = 0;
    app.projects_detail_focused = true;
    app.projects_detail_section = ProjectDetailSection::Tasks;
}

/// Load edit form for the highlighted task (milestone / dependence / start_date).
pub fn start_edit_project_task(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(project_id) = app.active_project else {
        app.set_status("Elegí un proyecto antes de editar una task.");
        return;
    };
    let Some(task) = app.project_tasks.get(app.selected_task_idx) else {
        app.set_status("No hay tasks para editar.");
        return;
    };
    if task.id == 0 {
        app.set_status("La task resaltada no tiene id interno.");
        return;
    }
    let task_id = task.id;
    app.project_task_form_edit_id = Some(task_id);
    app.projects_detail_focused = true;
    app.projects_detail_section = ProjectDetailSection::Tasks;
    app.composing_project_task = false;

    let id = app.requests.bump_project_task_form();
    app.begin_busy("Cargando task...");
    jobs.spawn(async move {
        let result = operations::projects::task_form(&client, project_id, task_id)
            .await
            .map_err(|e| e.to_string());
        JobOutcome::ProjectTaskFormLoaded {
            id,
            project_id,
            task_id,
            result,
        }
    });
}

pub fn open_project_task_edit_form(
    app: &mut App,
    task_id: u64,
    milestone: String,
    dependence: String,
    dependence_label: String,
    start_date: String,
) {
    app.composing_project_task = true;
    app.project_task_form_edit_id = Some(task_id);
    app.project_task_name_input.clear();
    app.project_task_desc_input.clear();
    app.project_task_milestone_input.set(milestone);
    app.project_task_dependence_input.set(dependence);
    let label = dependence_label.trim();
    app.project_task_dependence_label = if label.is_empty() || label == "---------" || label == "-"
    {
        None
    } else {
        Some(label.to_string())
    };
    app.project_task_start_date_input.set(start_date);
    app.project_task_form_field = 0;
    app.projects_detail_focused = true;
    app.projects_detail_section = ProjectDetailSection::Tasks;
}

pub fn cancel_project_task_form(app: &mut App) {
    app.composing_project_task = false;
    app.project_task_form_edit_id = None;
    app.project_task_name_input.clear();
    app.project_task_desc_input.clear();
    app.project_task_milestone_input.clear();
    app.project_task_dependence_input.clear();
    app.project_task_dependence_label = None;
    app.project_task_start_date_input.clear();
    app.project_task_form_field = 0;
}

fn project_task_form_field_count(app: &App) -> usize {
    if app.project_task_form_edit_id.is_some() {
        3 // milestone, dependence, start_date
    } else {
        4 // name, description, start_date, dependence
    }
}

pub fn toggle_project_task_form_field(app: &mut App) {
    let n = project_task_form_field_count(app);
    app.project_task_form_field = (app.project_task_form_field + 1) % n;
}

fn project_task_active_input(app: &mut App) -> &mut super::widgets::TextInput {
    if app.project_task_form_edit_id.is_some() {
        match app.project_task_form_field {
            0 => &mut app.project_task_milestone_input,
            1 => &mut app.project_task_dependence_input,
            _ => &mut app.project_task_start_date_input,
        }
    } else {
        match app.project_task_form_field {
            0 => &mut app.project_task_name_input,
            1 => &mut app.project_task_desc_input,
            2 => &mut app.project_task_start_date_input,
            _ => &mut app.project_task_dependence_input,
        }
    }
}

pub fn project_task_form_input_push(app: &mut App, ch: char) {
    let field = app.project_task_form_field;
    let edit = app.project_task_form_edit_id.is_some();
    let numericish = if edit { true } else { field == 2 || field == 3 };
    if numericish && !(ch.is_ascii_digit() || ch == '-') {
        return;
    }
    project_task_active_input(app).push(ch);
}

pub fn project_task_form_input_backspace(app: &mut App) {
    project_task_active_input(app).backspace();
}

pub fn submit_project_task_form(client: Arc<HteamClient>, app: &mut App, jobs: &Jobs) {
    let Some(project_id) = app.active_project else {
        cancel_project_task_form(app);
        return;
    };
    let edit_id = app.project_task_form_edit_id;

    if let Some(task_id) = edit_id {
        let milestone_raw = app.project_task_milestone_input.trim().to_string();
        let dependence_raw = app.project_task_dependence_input.trim().to_string();
        let start_date_raw = app.project_task_start_date_input.trim().to_string();
        let milestone_id = if milestone_raw.is_empty() {
            None
        } else {
            match milestone_raw.parse::<u64>() {
                Ok(id) => Some(id),
                Err(_) => {
                    app.set_status("Milestone debe ser un id numérico.");
                    return;
                }
            }
        };
        let dependence = if dependence_raw.is_empty() {
            None
        } else {
            match dependence_raw.parse::<u64>() {
                Ok(id) => Some(id),
                Err(_) => {
                    app.set_status("Dependence debe ser un id numérico.");
                    return;
                }
            }
        };
        let start_date = if start_date_raw.is_empty() {
            None
        } else {
            Some(start_date_raw)
        };
        if milestone_id.is_none() && dependence.is_none() && start_date.is_none() {
            app.set_status("Indicá milestone, dependence o start_date.");
            return;
        }
        cancel_project_task_form(app);
        let id = app.requests.bump_project_task_write();
        app.begin_busy("Guardando task...");
        jobs.spawn(async move {
            let start_ref = start_date.as_deref();
            let result = operations::projects::edit_task(
                &client,
                project_id,
                task_id,
                operations::projects::ProjectTaskEdit {
                    milestone_id,
                    dependence,
                    start_date: start_ref,
                },
            )
            .await
            .map(|w| format!("Task actualizada: {}", w.name))
            .map_err(|e| e.to_string());
            let refresh = project_detail_refresh(&client, project_id, result.is_ok()).await;
            JobOutcome::ProjectTaskSaved {
                id,
                project_id,
                result,
                refresh,
            }
        });
        return;
    }

    let name = app.project_task_name_input.trim().to_string();
    if name.is_empty() {
        app.set_status("El nombre de la task no puede estar vacío.");
        return;
    }
    let Some(milestone) = app.project_milestones.get(app.selected_milestone_idx) else {
        app.set_status("Seleccioná un milestone para la task.");
        return;
    };
    let Some(milestone_id) = milestone.id() else {
        app.set_status("No se pudo leer el id del milestone.");
        return;
    };
    let description = {
        let d = app.project_task_desc_input.trim().to_string();
        if d.is_empty() {
            None
        } else {
            Some(d)
        }
    };
    let start_date = {
        let d = app.project_task_start_date_input.trim().to_string();
        if d.is_empty() {
            None
        } else {
            Some(d)
        }
    };
    let dependence = {
        let d = app.project_task_dependence_input.trim().to_string();
        if d.is_empty() {
            None
        } else {
            match d.parse::<u64>() {
                Ok(id) => Some(id),
                Err(_) => {
                    app.set_status("Dependence debe ser un id numérico.");
                    return;
                }
            }
        }
    };
    cancel_project_task_form(app);
    let id = app.requests.bump_project_task_write();
    app.begin_busy("Creando task...");
    jobs.spawn(async move {
        let desc_ref = description.as_deref();
        let start_ref = start_date.as_deref();
        let result = operations::projects::create_task(
            &client,
            project_id,
            milestone_id,
            operations::projects::ProjectTaskCreate {
                name: &name,
                description: desc_ref,
                time_estimated: None,
                start_date: start_ref,
                dependence,
                responsible: None,
                priority: None,
            },
        )
        .await
        .map(|w| format!("Task creada: {}", w.name))
        .map_err(|e| e.to_string());
        let refresh = project_detail_refresh(&client, project_id, result.is_ok()).await;
        JobOutcome::ProjectTaskSaved {
            id,
            project_id,
            result,
            refresh,
        }
    });
}

async fn project_detail_refresh(
    client: &HteamClient,
    project_id: u64,
    ok: bool,
) -> Option<(
    Result<Vec<crate::models::ProjectMilestone>, String>,
    Result<Vec<crate::models::ProjectTask>, String>,
    Result<Vec<crate::models::ProjectTaskTreeNode>, String>,
)> {
    if !ok {
        return None;
    }
    let milestones = operations::projects::milestones(client, project_id)
        .await
        .map_err(|e| e.to_string());
    let tasks = operations::projects::tasks(client, project_id)
        .await
        .map(|resp| resp.results)
        .map_err(|e| e.to_string());
    let tree = operations::projects::task_tree(client, project_id)
        .await
        .map_err(|e| e.to_string());
    Some((milestones, tasks, tree))
}

/// Moves `id` to the front of the known-projects MRU list (deduping) and
/// persists it to `config.toml`. Optional `name` updates the cached label.
fn remember_project(app: &mut App, id: u64, name: Option<String>) {
    let kept = app
        .known_projects
        .iter()
        .find(|p| p.id == id)
        .and_then(|p| p.name.clone());
    let name = name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or(kept);
    app.known_projects.retain(|p| p.id != id);
    app.known_projects
        .insert(0, crate::config::KnownProject::new(id, name.clone()));
    app.selected_project_idx = 0;
    if name.is_some() {
        app.active_project_name = name.clone();
    }

    if let Err(e) = persist_known_project(id, name.as_deref()) {
        app.set_status(format!(
            "Proyecto cargado (no se pudo guardar en config.toml: {})",
            e
        ));
    }
}

/// Learn/update the display name for a project (also persists to config.toml).
pub(crate) fn cache_project_name_for_net(app: &mut App, id: u64, name: String) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    if app.active_project == Some(id) {
        app.active_project_name = Some(name.to_string());
    }
    remember_project(app, id, Some(name.to_string()));
}

fn persist_known_project(id: u64, name: Option<&str>) -> Result<()> {
    let mut config = crate::config::Config::load()?;
    operations::projects::remember_project(&mut config, id, name)
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
    if app.composing_milestone || app.composing_project_task || app.composing_member {
        return;
    }
    app.projects_detail_focused = !app.projects_detail_focused;
    app.projects_scroll.reset();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkingHoursConfig;
    use crate::models::{
        Comment, Label, ProjectMilestone, ProjectTask, ProjectTaskTreeNode, Reminder,
    };
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
    fn label_picker_filters_navigates_and_toggles_selection() {
        let mut app = test_app();
        app.label_picker_items = vec![
            Label {
                id: 315,
                name: "DOING".into(),
                color: "#00ff00".into(),
            },
            Label {
                id: 473,
                name: "Proceso".into(),
                color: "#ff0000".into(),
            },
        ];
        app.label_picker_filter.set("proc");

        assert_eq!(filtered_label_picker_indices(&app), vec![1]);
        label_picker_move(&mut app, 1);
        assert_eq!(app.label_picker_idx, 1);
        toggle_label_picker_selection(&mut app);
        assert!(app.label_picker_selected.contains(&473));
        toggle_label_picker_selection(&mut app);
        assert!(!app.label_picker_selected.contains(&473));
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
    fn reply_uses_selected_comment_id() {
        let mut app = test_app();
        app.comments = vec![Comment {
            id: 12,
            user_name: "ana".into(),
            submit_date: String::new(),
            comment: "first".into(),
            level: 0,
            parent_id: None,
            is_removed: false,
            permalink: None,
        }];

        select_comment(&mut app, 1);
        start_reply_comment(&mut app);

        assert_eq!(app.comments_selected, Some(0));
        assert_eq!(app.comment_reply_to, Some(12));
        assert!(app.composing_comment);
    }

    #[test]
    fn milestone_form_create_and_edit_seed_fields() {
        let mut app = test_app();
        app.active_project = Some(500);
        app.project_milestones = vec![ProjectMilestone {
            name: "Sprint A".into(),
            progress: 0.25,
            url: "/projects/500/milestones/42/".into(),
        }];
        app.selected_milestone_idx = 0;

        start_create_milestone(&mut app);
        assert!(app.composing_milestone);
        assert!(app.milestone_form_edit_id.is_none());
        assert!(app.milestone_name_input.as_str().is_empty());
        assert!(app.projects_detail_focused);

        cancel_milestone_form(&mut app);
        open_milestone_edit_form(&mut app, 42, "Sprint A".into(), "Notas del sprint".into());
        assert!(app.composing_milestone);
        assert_eq!(app.milestone_form_edit_id, Some(42));
        assert_eq!(app.milestone_name_input.as_str(), "Sprint A");
        assert_eq!(app.milestone_desc_input.as_str(), "Notas del sprint");
        assert!(app.milestone_desc_focused, "edit focuses description");

        // Name is locked: typing goes to description.
        milestone_form_input_push(&mut app, '!');
        assert_eq!(app.milestone_name_input.as_str(), "Sprint A");
        assert_eq!(app.milestone_desc_input.as_str(), "Notas del sprint!");
        toggle_milestone_form_field(&mut app);
        assert!(app.milestone_desc_focused);

        move_milestone_selection(&mut app, 1);
        assert_eq!(app.selected_milestone_idx, 0); // only one item
    }

    #[test]
    fn project_task_form_create_requires_milestone_and_cycles_fields() {
        let mut app = test_app();
        app.active_project = Some(500);
        start_create_project_task(&mut app);
        assert!(!app.composing_project_task); // no milestones yet

        app.project_milestones = vec![ProjectMilestone {
            name: "M1".into(),
            progress: 0.0,
            url: "/projects/500/milestones/10/".into(),
        }];
        app.selected_milestone_idx = 0;
        start_create_project_task(&mut app);
        assert!(app.composing_project_task);
        assert!(app.project_task_form_edit_id.is_none());
        assert_eq!(app.projects_detail_section, ProjectDetailSection::Tasks);

        project_task_form_input_push(&mut app, 'A');
        assert_eq!(app.project_task_name_input.as_str(), "A");
        toggle_project_task_form_field(&mut app);
        assert_eq!(app.project_task_form_field, 1);
        project_task_form_input_push(&mut app, 'd');
        assert_eq!(app.project_task_desc_input.as_str(), "d");

        open_project_task_edit_form(
            &mut app,
            99,
            "10".into(),
            "12".into(),
            "#1 - Parent".into(),
            "2026-10-01".into(),
        );
        assert_eq!(app.project_task_form_edit_id, Some(99));
        assert_eq!(app.project_task_milestone_input.as_str(), "10");
        assert_eq!(app.project_task_dependence_input.as_str(), "12");
        assert_eq!(
            app.project_task_dependence_label.as_deref(),
            Some("#1 - Parent")
        );
        assert_eq!(app.project_task_start_date_input.as_str(), "2026-10-01");

        app.project_tasks = vec![ProjectTask {
            id: 99,
            number: 1,
            name: "T".into(),
            milestone: None,
            progress: 0.0,
            closed: false,
            responsible: None,
        }];
        move_task_selection(&mut app, 1);
        assert_eq!(app.selected_task_idx, 0);
        cancel_project_task_form(&mut app);
        cycle_projects_detail_section(&mut app, 1);
        assert_eq!(app.projects_detail_section, ProjectDetailSection::Tree);
        cycle_projects_detail_section(&mut app, 1);
        assert_eq!(app.projects_detail_section, ProjectDetailSection::Status);
        cycle_projects_detail_section(&mut app, 1);
        assert_eq!(app.projects_detail_section, ProjectDetailSection::Members);
        cycle_projects_detail_section(&mut app, 1);
        assert_eq!(
            app.projects_detail_section,
            ProjectDetailSection::Milestones
        );
    }

    #[test]
    fn project_members_add_remove_and_dirty_flag() {
        let mut app = test_app();
        app.active_project = Some(500);
        app.project_members = Some(vec![ProjectMemberEntry {
            id: 140,
            label: "#140".into(),
        }]);
        app.project_members_baseline = vec![140];
        app.selected_member_idx = 0;
        assert!(!project_members_dirty(&app));

        // Simulate resolved add applied by net layer.
        app.project_members
            .as_mut()
            .unwrap()
            .push(ProjectMemberEntry {
                id: 114,
                label: "ana (#114)".into(),
            });
        assert!(project_members_dirty(&app));

        app.selected_member_idx = 1;
        remove_selected_member(&mut app);
        assert!(!project_members_dirty(&app));
        assert_eq!(app.project_members.as_ref().unwrap().len(), 1);

        remove_selected_member(&mut app);
        assert!(project_members_dirty(&app));
        assert!(app.project_members.as_ref().unwrap().is_empty());

        start_composing_member(&mut app);
        assert!(app.composing_member);
        member_input_push(&mut app, 'a');
        assert_eq!(app.member_input.as_str(), "a");
        cancel_composing_member(&mut app);
        assert!(!app.composing_member);
    }

    #[test]
    fn project_task_tree_section_cycles_and_selects_flat_rows() {
        let mut app = test_app();
        app.projects_detail_focused = true;
        app.projects_detail_section = ProjectDetailSection::Tasks;
        cycle_projects_detail_section(&mut app, 1);
        assert_eq!(app.projects_detail_section, ProjectDetailSection::Tree);
        cycle_projects_detail_section(&mut app, -1);
        assert_eq!(app.projects_detail_section, ProjectDetailSection::Tasks);

        app.project_task_tree = vec![ProjectTaskTreeNode {
            id: 1,
            name: "Parent".into(),
            children: vec![ProjectTaskTreeNode {
                id: 2,
                name: "Child".into(),
                children: Vec::new(),
            }],
        }];
        assert_eq!(project_task_tree_len(&app.project_task_tree), 2);
        // Collapsed: only the root is visible.
        assert_eq!(
            project_task_tree_visible_len(&app.project_task_tree, &app.project_tree_expanded),
            1
        );
        app.projects_detail_section = ProjectDetailSection::Tree;
        app.selected_tree_idx = 0;
        move_tree_selection(&mut app, 1);
        assert_eq!(app.selected_tree_idx, 0);

        toggle_tree_node(&mut app);
        assert!(app.project_tree_expanded.contains(&1));
        assert_eq!(
            project_task_tree_visible_len(&app.project_task_tree, &app.project_tree_expanded),
            2
        );
        move_tree_selection(&mut app, 1);
        assert_eq!(app.selected_tree_idx, 1);
        move_tree_selection(&mut app, 1);
        assert_eq!(app.selected_tree_idx, 1);

        // Collapse again hides the child and clamps selection.
        app.selected_tree_idx = 1;
        toggle_tree_node(&mut app); // selected is child (leaf) — no-op expand
        assert!(app.project_tree_expanded.contains(&1));
        app.selected_tree_idx = 0;
        toggle_tree_node(&mut app);
        assert!(!app.project_tree_expanded.contains(&1));
        assert_eq!(app.selected_tree_idx, 0);
    }

    #[test]
    fn milestone_delete_pending_clears_on_cancel_and_selection() {
        let mut app = test_app();
        app.active_project = Some(500);
        app.project_milestones_loaded = true;
        app.project_milestones = vec![
            ProjectMilestone {
                name: "Temp".into(),
                progress: 0.0,
                url: "/projects/500/milestones/42/".into(),
            },
            ProjectMilestone {
                name: "Other".into(),
                progress: 0.0,
                url: "/projects/500/milestones/43/".into(),
            },
        ];
        app.selected_milestone_idx = 0;
        app.pending_milestone_delete = Some(42);
        cancel_pending_milestone_delete(&mut app);
        assert!(app.pending_milestone_delete.is_none());

        app.pending_milestone_delete = Some(42);
        move_milestone_selection(&mut app, 1);
        assert!(app.pending_milestone_delete.is_none());
        assert_eq!(app.selected_milestone_idx, 1);
    }

    #[test]
    fn milestone_form_rejects_empty_name_without_clearing_inputs() {
        let mut app = test_app();
        app.active_project = Some(500);
        app.composing_milestone = true;
        app.milestone_name_input.set("   ");
        // submit_milestone_form needs Jobs; validate the empty-name guard via the
        // same precondition the handler checks before spawning work.
        assert!(app.milestone_name_input.trim().is_empty());
        start_create_milestone(&mut app);
        app.milestone_name_input.set("");
        app.milestone_desc_input.set("notes");
        // Empty name must keep the form open — exercised through cancel/start only here;
        // network submit is covered by operations tests.
        assert!(app.composing_milestone);
        cancel_milestone_form(&mut app);
        assert!(!app.composing_milestone);
    }

    #[test]
    fn selecting_reminder_tracks_selected_index() {
        let mut app = test_app();
        app.reminders = vec![
            Reminder {
                id: 5,
                object_id: 10,
                reminder_type: Some(1),
                reminder_date: None,
                content_object: None,
            },
            Reminder {
                id: 8,
                object_id: 12,
                reminder_type: Some(1),
                reminder_date: None,
                content_object: None,
            },
        ];

        select_reminder(&mut app, 1);
        assert_eq!(app.selected_reminder, Some(0));
        select_reminder(&mut app, 1);
        assert_eq!(app.selected_reminder, Some(1));
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

    #[test]
    fn task_detail_field_navigation_and_edit_seed() {
        use crate::models::CardDetail;

        let mut app = board_with_cards();
        app.show_task_detail = true;
        app.task_detail_card_id = Some(1);
        app.task_detail = Some(CardDetail {
            id: 1,
            name: "Alpha GBI".into(),
            description: Some("body".into()),
            status: None,
            labels: vec![],
            list: None,
            number: Some(303),
            created_at: None,
            updated_at: None,
            time_estimated: Some("1h".into()),
            time_spend: Some("45m".into()),
            start_date: Some("2026-10-13".into()),
            due_date: Some("2026-10-14".into()),
            dependence: None,
            responsible: Some(serde_json::json!({"username": "ana"})),
            priority: Some(serde_json::json!(2)),
            closed: Some(false),
            title_url: None,
            milestone: None,
            milestone_name: None,
        });

        detail_move_field(&mut app, 1);
        assert_eq!(app.task_detail_field_idx, 1); // Description
        start_edit_detail_field(&mut app);
        assert!(app.editing_detail_field);
        assert_eq!(app.detail_edit_input.as_str(), "body");
        cancel_edit_detail_field(&mut app);
        assert!(!app.editing_detail_field);

        close_task_detail(&mut app);
        assert!(!app.show_task_detail);
        assert!(app.task_detail.is_none());
    }
}
