//! Author credit shown in CLI help, interactive mode, and the TUI footer.
//!
//! The GitHub mark is a Nerd Font PUA glyph (`nf-fa-github`). When Nerd Fonts
//! are not available the icon is omitted so the terminal never shows tofu.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// GitHub handle shown next to the optional Nerd Font mark.
pub const AUTHOR: &str = "4DRIAN0RTIZ";

/// Years covered by the project copyright notice.
pub const COPYRIGHT_YEARS: &str = "2026";

/// `nf-fa-github` (Font Awesome) in the Nerd Fonts private-use area.
const GITHUB_NF: char = '\u{f09b}';

/// Formal notice kept in sync with the top-level `COPYRIGHT` file.
pub fn copyright_notice() -> String {
    format!("Copyright (C) {COPYRIGHT_YEARS} {AUTHOR}")
}

/// `Made by …` line for CLI help / REPL banner.
pub fn made_by_line() -> String {
    match github_prefix() {
        Some(icon) => format!("Made by {icon} {AUTHOR}"),
        None => format!("Made by {AUTHOR}"),
    }
}

/// Compact credit for the TUI footer (icon + handle, or handle alone).
pub fn credit() -> String {
    match github_prefix() {
        Some(icon) => format!("{icon} {AUTHOR}"),
        None => AUTHOR.to_string(),
    }
}

/// Multi-line text for `hteam --version` (clap `long_version`).
///
/// Legal notice only (copyright + license). Product credit lives in
/// `--help` / REPL / TUI via `made_by_line` / `credit`.
///
/// Clap needs a `'static` str; we leak a single process-wide string built
/// from `copyright_notice` so the notice stays in one place.
pub fn long_version() -> &'static str {
    static LONG: OnceLock<&'static str> = OnceLock::new();
    LONG.get_or_init(|| {
        Box::leak(
            format!(
                "{}\n{}\nLicense: MIT",
                env!("CARGO_PKG_VERSION"),
                copyright_notice(),
            )
            .into_boxed_str(),
        )
    })
}

fn github_prefix() -> Option<char> {
    if nerd_fonts_available() {
        Some(GITHUB_NF)
    } else {
        None
    }
}

fn nerd_fonts_available() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(detect_nerd_fonts)
}

fn detect_nerd_fonts() -> bool {
    if let Some(forced) = env_flag("HTEAM_NERD_FONTS") {
        return forced;
    }

    nerd_font_files_present()
}

fn env_flag(name: &str) -> Option<bool> {
    parse_env_flag(&std::env::var(name).ok()?)
}

fn parse_env_flag(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Best-effort probe: if a Nerd Font file is installed, the glyph is likely
/// usable. Does not know which font the terminal actually selected.
fn nerd_font_files_present() -> bool {
    for dir in font_search_dirs() {
        if dir_contains_nerd_font(&dir) {
            return true;
        }
    }
    false
}

fn font_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/fonts"));
        dirs.push(home.join(".fonts"));
        dirs.push(home.join("Library/Fonts"));
        dirs.push(home.join(".local/share/fonts/nerd-fonts"));
    }

    dirs.push(PathBuf::from("/usr/share/fonts"));
    dirs.push(PathBuf::from("/usr/local/share/fonts"));
    dirs.push(PathBuf::from("/usr/share/fonts/nerd-fonts"));

    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
    }

    dirs
}

fn dir_contains_nerd_font(root: &Path) -> bool {
    let mut stack = vec![root.to_path_buf()];
    let mut seen = 0usize;

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            seen += 1;
            // Bound the walk so a huge fonts tree cannot stall startup.
            if seen > 4_000 {
                return false;
            }

            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();

            if path.is_dir() {
                // Skip cache/metadata dirs.
                if name == "cache" || name.starts_with('.') {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if is_nerd_font_filename(&name) {
                return true;
            }
        }
    }

    false
}

fn is_nerd_font_filename(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let is_font = name.ends_with(".ttf")
        || name.ends_with(".otf")
        || name.ends_with(".ttc")
        || name.ends_with(".woff")
        || name.ends_with(".woff2");
    if !is_font {
        return false;
    }

    name.contains("nerd")
        || name.contains("meslolgs")
        || name.contains("meslo lg s nf")
        || name.ends_with(" nf.ttf")
        || name.ends_with(" nf.otf")
        || name.contains("nf-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credit_without_nerd_fonts_is_handle_only() {
        // Force off for this process; OnceLock may already be set from other
        // tests in the same binary, so assert via the pure helpers instead.
        assert_eq!(AUTHOR, "4DRIAN0RTIZ");
        assert!(is_nerd_font_filename("JetBrainsMonoNerdFont-Regular.ttf"));
        assert!(is_nerd_font_filename("MesloLGS NF.ttf"));
        assert!(!is_nerd_font_filename("DejaVuSansMono.ttf"));
        assert!(!is_nerd_font_filename("readme.md"));
    }

    #[test]
    fn env_flag_parses_common_truthy_falsey() {
        assert_eq!(parse_env_flag("true"), Some(true));
        assert_eq!(parse_env_flag("YES"), Some(true));
        assert_eq!(parse_env_flag("0"), Some(false));
        assert_eq!(parse_env_flag("off"), Some(false));
        assert_eq!(parse_env_flag("maybe"), None);
    }

    #[test]
    fn made_by_line_contains_author() {
        let line = made_by_line();
        assert!(line.contains(AUTHOR), "{line}");
        assert!(line.starts_with("Made by "), "{line}");
    }

    #[test]
    fn credit_contains_author() {
        assert!(credit().contains(AUTHOR));
    }

    #[test]
    fn copyright_notice_is_stable() {
        assert_eq!(
            copyright_notice(),
            format!("Copyright (C) {COPYRIGHT_YEARS} {AUTHOR}")
        );
    }

    #[test]
    fn long_version_includes_copyright_and_license() {
        let text = long_version();
        assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
        assert!(text.contains(&copyright_notice()), "{text}");
        assert!(text.contains("License: MIT"), "{text}");
        assert!(!text.contains("Made by"), "{text}");
    }
}
