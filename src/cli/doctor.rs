use anyhow::Result;
use colored::Colorize;

use crate::operations::doctor::{self, CheckStatus, DoctorReport};

pub async fn execute(json: bool, board: Option<u64>) -> Result<()> {
    let report = doctor::run(board).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human(&report);
    }

    if report.ok {
        Ok(())
    } else {
        anyhow::bail!("doctor found problems — see checks above")
    }
}

fn print_human(report: &DoctorReport) {
    println!("hteam doctor\n");

    if let Some(path) = &report.config_path {
        println!("config: {path}");
    }
    match (report.board, &report.board_name) {
        (Some(id), Some(name)) => println!("board:  {name} ({id})"),
        (Some(id), None) => println!("board:  {id}"),
        _ => {}
    }
    if let Some(n) = report.boards_visible {
        println!("boards visible: {n}");
    }
    println!();

    for check in &report.checks {
        let mark = match check.status {
            CheckStatus::Ok => "ok".green().bold(),
            CheckStatus::Warn => "warn".yellow().bold(),
            CheckStatus::Fail => "fail".red().bold(),
        };
        println!("[{mark}] {}: {}", check.id, check.message);
        if let Some(hint) = &check.hint {
            println!("       → {hint}");
        }
    }

    println!();
    if report.ok {
        println!("{}", "All critical checks passed.".green());
    } else {
        println!("{}", "One or more checks failed.".red());
    }
}
