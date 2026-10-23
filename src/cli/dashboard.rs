use anyhow::Result;
use colored::Colorize;
use tabled::{settings::Style, Table};

use crate::cli::objectives::relative_time;
use crate::models::{Reminder, WorkingOnStatus};
use crate::operations::dashboard::{Dashboard, DashboardObjectives};
use crate::operations::Session;

pub async fn execute(json: bool) -> Result<()> {
    let mut session = Session::open().await?;
    let dashboard = crate::operations::dashboard::load(&session.client, &mut session.config).await;

    if dashboard.objectives.is_ok() {
        if let Some(objectives) = dashboard.objectives.as_ok() {
            if !objectives.from_cache {
                session.config.save()?;
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&dashboard)?);
        return Ok(());
    }

    print_human(&dashboard);
    Ok(())
}

fn print_human(dashboard: &Dashboard) {
    println!("\n{}\n", "Dashboard".bold());

    println!("{}", "Working on it".bold());
    if let Some(items) = dashboard.working.as_ok() {
        print_working(items);
    } else if let Some(error) = dashboard.working.error() {
        print_section_error(error);
    }

    println!("{}", "Recordatorios".bold());
    if let Some(items) = dashboard.reminders.as_ok() {
        print_reminders(items);
    } else if let Some(error) = dashboard.reminders.error() {
        print_section_error(error);
    }

    println!("{}", "Objetivos semanales".bold());
    if let Some(view) = dashboard.objectives.as_ok() {
        print_objectives(view);
    } else if let Some(error) = dashboard.objectives.error() {
        print_section_error(error);
    }
}

fn print_section_error(error: &str) {
    println!("  {} {}\n", "error:".red().bold(), error);
}

fn print_working(items: &[WorkingOnStatus]) {
    if items.is_empty() {
        println!("  No estás trabajando en ningún card.\n");
        return;
    }
    let mut table = Table::new(items);
    table.with(Style::rounded());
    println!("{table}\n");
}

fn print_reminders(items: &[Reminder]) {
    if items.is_empty() {
        println!("  No tienes recordatorios pendientes.\n");
        return;
    }
    let mut table = Table::new(items);
    table.with(Style::rounded());
    println!("{table}\n");
}

fn print_objectives(view: &DashboardObjectives) {
    let source = if view.from_cache { "cache" } else { "red" };
    println!(
        "  {} ({}, {})",
        view.set.title.bold(),
        source,
        relative_time(view.synced_at)
    );

    if view.set.objectives.is_empty() {
        println!("  (sin objetivos)\n");
        return;
    }

    for objective in &view.set.objectives {
        println!("  ## {}", objective.name);
        if objective.tasks.is_empty() {
            println!("     (sin tareas)");
        }
        for task in &objective.tasks {
            let mark = if task.done { "x".green() } else { " ".normal() };
            println!("     [{}] {}", mark, task.description);
        }
    }
    println!();
}
