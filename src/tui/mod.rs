mod app;
mod events;
pub mod theme;
mod ui;
mod widgets;

use std::collections::HashSet;
use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::client::HteamClient;
use crate::operations::{self, Session};

use app::App;
use theme::Theme;

struct TerminalGuard {
    /// Stderr original, respaldado mientras dura la pantalla alterna — ver
    /// `redirect_stderr_to_log`. `None` en plataformas no-unix o si el
    /// respaldo falló (en ese caso no hay nada que restaurar).
    #[cfg(unix)]
    saved_stderr: Option<std::os::fd::RawFd>,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        #[cfg(unix)]
        let saved_stderr = redirect_stderr_to_log().ok();

        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;

        #[cfg(unix)]
        return Ok(Self { saved_stderr });
        #[cfg(not(unix))]
        return Ok(Self {});
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);

        #[cfg(unix)]
        if let Some(saved) = self.saved_stderr.take() {
            unsafe {
                libc::dup2(saved, libc::STDERR_FILENO);
                libc::close(saved);
            }
        }
    }
}

/// Algunas dependencias (p.ej. `arboard` → `wl-clipboard-rs`, que sirve el
/// portapapeles Wayland desde un hilo en background) escriben directo a
/// stderr con `eprintln!` o un panic en vez de pasar por `log`. Sobre la
/// pantalla alterna en raw mode eso se pinta encima del frame de ratatui y
/// queda ahí hasta que esa zona se redibuje. Redirigimos stderr al mismo
/// `tui.log` que ya usa `App::set_status` mientras dura la sesión del TUI,
/// y lo restauramos al salir.
#[cfg(unix)]
fn redirect_stderr_to_log() -> Result<std::os::fd::RawFd> {
    use std::os::fd::AsRawFd;

    let dir = crate::config::Config::config_dir()?;
    std::fs::create_dir_all(&dir)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("tui.log"))
        .context("No se pudo abrir tui.log para redirigir stderr")?;

    let saved = unsafe { libc::dup(libc::STDERR_FILENO) };
    if saved < 0 {
        anyhow::bail!(
            "No se pudo respaldar stderr: {}",
            io::Error::last_os_error()
        );
    }

    if unsafe { libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO) } < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(saved) };
        anyhow::bail!("No se pudo redirigir stderr: {}", err);
    }

    Ok(saved)
}

pub async fn run(board: Option<u64>) -> Result<()> {
    let session = Session::open().await?;
    let board_number = operations::resolve_board_number(&session.config, board).context(
        "No se especificó el board number. Usa --board o configura uno con 'hteam board switch'.",
    )?;

    let hidden_lists: HashSet<String> = session
        .config
        .tui
        .hidden_lists
        .iter()
        .map(|s| s.to_lowercase())
        .collect();
    let known_projects = session.config.tui.known_projects.clone();
    let working_hours = session.config.working_hours.clone();
    let theme = Theme::from_name(&session.config.theme.active);

    let client = Arc::new(session.client);

    let mut app = App::new(
        board_number,
        hidden_lists,
        known_projects,
        working_hours,
        theme,
    );
    if let Some(status) = update_notice().await {
        app.set_status(status);
    }
    events::refresh_all(&client, &mut app).await;

    let guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, &client, &mut app).await;

    drop(guard);
    result
}

/// Chequea (con el mismo cache TTL que `hteam` usa en CLI) si hay una
/// versión nueva publicada en GitHub y si corresponde mostrar el changelog
/// post-actualización, devolviendo un mensaje corto para el footer del TUI
/// (`app.set_status`) en vez del changelog completo — no hay espacio para
/// volcarlo entero en una línea de estado. Usa su propio `Config::load()`/
/// `save()`, independiente de `session.config`, mismo patrón que el popup
/// de objetivos semanales. Cualquier error (config corrupta, sin red) se
/// ignora en silencio: nunca debe impedir que el TUI arranque.
async fn update_notice() -> Option<String> {
    use crate::config::Config;

    let mut config = Config::load().ok()?;
    let mut dirty = false;
    let mut parts: Vec<String> = Vec::new();

    if operations::update::resolve_startup_changelog(
        &mut config,
        operations::update::EMBEDDED_CHANGELOG,
    )
    .is_some()
    {
        parts.push("hteam se actualizó, ver CHANGELOG.md".to_string());
        dirty = true;
    }

    if let Ok(result) =
        operations::update::check_for_update(&mut config, crate::client::update::GITHUB_API_BASE)
            .await
    {
        dirty = dirty || !result.from_cache;

        if result.update_available {
            parts.push(format!(
                "nueva versión {} disponible, corré `hteam update`",
                result.latest_version
            ));
        }
    }

    if dirty {
        let _ = config.save();
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

/// How long a footer confirmation/error message (e.g. "Descripción
/// actualizada.") stays up before the footer reverts to the keybindings hint.
const STATUS_TTL: Duration = Duration::from_secs(3);

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    client: &Arc<HteamClient>,
    app: &mut App,
) -> Result<()> {
    loop {
        app.expire_status(STATUS_TTL);
        terminal.draw(|f| ui::draw(f, app))?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if app.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => events::close_help(app),
                KeyCode::Char('j') | KeyCode::Down => app.help_scroll.by(1),
                KeyCode::Char('k') | KeyCode::Up => app.help_scroll.by(-1),
                _ => {}
            }
            continue;
        }

        if app.show_reminders {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => events::close_reminders(app),
                KeyCode::Char('a') => events::add_reminder_for_current_card(client, app).await,
                KeyCode::Char('j') | KeyCode::Down => app.reminders_scroll.by(1),
                KeyCode::Char('k') | KeyCode::Up => app.reminders_scroll.by(-1),
                _ => {}
            }
            continue;
        }

        if app.show_weekly_objectives {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => events::close_weekly_objectives(app),
                KeyCode::Char('f') => events::open_weekly_objectives(app, true).await,
                KeyCode::Char('j') | KeyCode::Down => app.weekly_objectives_scroll.by(1),
                KeyCode::Char('k') | KeyCode::Up => app.weekly_objectives_scroll.by(-1),
                _ => {}
            }
            continue;
        }

        if app.show_description {
            match key.code {
                KeyCode::Esc => events::close_description(app),
                KeyCode::Enter => app.description_input.push('\n'),
                KeyCode::Backspace => events::description_input_backspace(app),
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    events::submit_description(client, app).await;
                }
                KeyCode::Char(c) => events::description_input_push(app, c),
                _ => {}
            }
            continue;
        }

        if app.show_comments {
            if app.composing_comment {
                match key.code {
                    KeyCode::Esc => {
                        if !app.mention_suggestions.is_empty() {
                            app.mention_suggestions.clear();
                        } else {
                            events::cancel_composing_comment(app);
                        }
                    }
                    KeyCode::Enter => {
                        if !app.mention_suggestions.is_empty() {
                            events::accept_mention_suggestion(app);
                        } else if !app.comment_date_focused {
                            app.comment_input.push('\n');
                        }
                    }
                    KeyCode::Down => events::mention_move_selection(app, 1),
                    KeyCode::Up => events::mention_move_selection(app, -1),
                    KeyCode::Tab => events::toggle_comment_date_focus(app),
                    KeyCode::Backspace => {
                        if app.comment_date_focused {
                            events::comment_date_input_backspace(app);
                        } else {
                            events::comment_input_backspace(client, app).await;
                        }
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        events::submit_comment(client, app).await;
                    }
                    KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        events::toggle_comment_follow(app);
                    }
                    KeyCode::Char(c) => {
                        if app.comment_date_focused {
                            events::comment_date_input_push(app, c);
                        } else {
                            events::comment_input_push(client, app, c).await;
                        }
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => events::close_comments(app),
                    KeyCode::Char('a') => events::start_composing_comment(app),
                    KeyCode::Char('j') | KeyCode::Down => app.comments_scroll.by(1),
                    KeyCode::Char('k') | KeyCode::Up => app.comments_scroll.by(-1),
                    KeyCode::Char('v') if app.current_follow_up.is_some() => {
                        events::complete_current_follow_up(client, app).await;
                    }
                    KeyCode::Char('x') if app.current_follow_up.is_some() => {
                        events::cancel_current_follow_up(client, app).await;
                    }
                    _ => {}
                }
            }
            continue;
        }

        if app.show_projects {
            if app.composing_project {
                match key.code {
                    KeyCode::Esc => events::cancel_composing_project(app),
                    KeyCode::Enter => events::submit_project_input(client, app).await,
                    KeyCode::Backspace => events::project_input_backspace(app),
                    KeyCode::Char(c) => events::project_input_push(app, c),
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => events::close_projects(app),
                    KeyCode::Char('a') => events::start_composing_project(app),
                    KeyCode::Tab => events::toggle_projects_focus(app),
                    KeyCode::Char('j') | KeyCode::Down => {
                        if app.projects_detail_focused {
                            app.projects_scroll.by(1);
                        } else {
                            events::move_project_selection(app, 1);
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if app.projects_detail_focused {
                            app.projects_scroll.by(-1);
                        } else {
                            events::move_project_selection(app, -1);
                        }
                    }
                    KeyCode::Enter if !app.projects_detail_focused => {
                        events::select_known_project(client, app).await;
                    }
                    KeyCode::Char('c') => events::copy_project_link(client, app),
                    _ => {}
                }
            }
            continue;
        }

        if app.show_board_switch {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => events::close_board_switch(app),
                KeyCode::Char('j') | KeyCode::Down => events::move_board_selection(app, 1),
                KeyCode::Char('k') | KeyCode::Up => events::move_board_selection(app, -1),
                KeyCode::Enter => events::select_current_board(client, app).await,
                KeyCode::Char('r') => events::open_board_switch(client, app).await,
                _ => {}
            }
            continue;
        }

        if app.composing_card {
            match key.code {
                KeyCode::Esc => events::cancel_composing_card(app),
                KeyCode::Enter => events::submit_new_card(client, app).await,
                KeyCode::Backspace => events::new_card_input_backspace(app),
                KeyCode::Char(c) => events::new_card_input_push(app, c),
                _ => {}
            }
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Char('j') | KeyCode::Down => app.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => app.move_selection(-1),
            KeyCode::Char('h') | KeyCode::Left => app.move_list(-1),
            KeyCode::Char('l') | KeyCode::Right => app.move_list(1),
            KeyCode::Char('H') => events::move_active_card(client, app, -1).await,
            KeyCode::Char('L') => events::move_active_card(client, app, 1).await,
            KeyCode::Char('v') => events::toggle_hide_current_list(app),
            KeyCode::Char('V') => events::show_all_lists(app),
            KeyCode::Char('R') => events::open_reminders(client, app).await,
            KeyCode::Char('O') => events::open_weekly_objectives(app, false).await,
            KeyCode::Char('C') => events::open_comments(client, app).await,
            KeyCode::Char('c') => events::copy_card_link(client, app).await,
            KeyCode::Char('P') => events::open_projects(client, app).await,
            KeyCode::Char('n') => events::start_composing_card(app),
            KeyCode::Char('w') => events::toggle_working_on(client, app).await,
            KeyCode::Char('r') => events::refresh_all(client, app).await,
            KeyCode::Char('B') => events::open_board_switch(client, app).await,
            KeyCode::Char('?') => events::open_help(app),
            KeyCode::Enter => events::open_description(app),
            _ => {}
        }
    }

    Ok(())
}
