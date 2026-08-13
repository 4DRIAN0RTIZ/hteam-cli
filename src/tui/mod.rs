mod app;
mod events;
mod ui;

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
use crate::config::Config;

use app::App;

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

pub async fn run(board: Option<u64>) -> Result<()> {
    let config = Config::load()?;
    let board_number = board
        .or(config.auth.board_number)
        .or_else(|| config.load_last_ticket().ok().flatten())
        .context("No se especificó el board number. Usa --board o configura uno con 'hteam board switch'.")?;

    let hidden_lists: HashSet<String> = config
        .tui
        .hidden_lists
        .iter()
        .map(|s| s.to_lowercase())
        .collect();
    let known_projects = config.tui.known_projects.clone();

    let client = Arc::new(HteamClient::with_auth(config).await?);

    let mut app = App::new(board_number, hidden_lists, known_projects);
    events::refresh_all(&client, &mut app).await;

    let guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, &client, &mut app).await;

    drop(guard);
    result
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
                _ => {}
            }
            continue;
        }

        if app.show_reminders {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => events::close_reminders(app),
                KeyCode::Char('a') => events::add_reminder_for_current_card(client, app).await,
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
                        } else {
                            app.comment_input.push('\n');
                        }
                    }
                    KeyCode::Down => events::mention_move_selection(app, 1),
                    KeyCode::Up => events::mention_move_selection(app, -1),
                    KeyCode::Backspace => events::comment_input_backspace(client, app).await,
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        events::submit_comment(client, app).await;
                    }
                    KeyCode::Char(c) => events::comment_input_push(client, app, c).await,
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => events::close_comments(app),
                    KeyCode::Char('a') => events::start_composing_comment(app),
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
                    KeyCode::Char('j') | KeyCode::Down => events::move_project_selection(app, 1),
                    KeyCode::Char('k') | KeyCode::Up => events::move_project_selection(app, -1),
                    KeyCode::Enter => events::select_known_project(client, app).await,
                    _ => {}
                }
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
            KeyCode::Char('C') => events::open_comments(client, app).await,
            KeyCode::Char('P') => events::open_projects(client, app).await,
            KeyCode::Char('n') => events::start_composing_card(app),
            KeyCode::Char('w') => events::toggle_working_on(client, app).await,
            KeyCode::Char('r') => events::refresh_all(client, app).await,
            KeyCode::Char('?') => events::open_help(app),
            KeyCode::Enter => events::open_description(app),
            _ => {}
        }
    }

    Ok(())
}
