//! Orquesta el autochequeo de nuevas versiones, la autoactualización del
//! binario y el aviso de changelog al arrancar tras un cambio de versión.
//! Sin I/O de red directo (eso vive en `client::update`); esta capa decide
//! cache/TTL, comparación semver simplificada y el flujo de reemplazo del
//! binario en disco.

use anyhow::{Context, Result};
use chrono::Utc;
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::client;
use crate::config::Config;

/// Changelog embebido en build time — la misma copia que el pipeline de
/// release genera con git-cliff y commitea antes de compilar el binario
/// publicado (ver `.github/workflows/release.yml`), así que cada binario
/// released trae su propio historial hasta esa versión.
pub const EMBEDDED_CHANGELOG: &str = include_str!("../../CHANGELOG.md");

/// Resultado de comparar la versión actual contra la última conocida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    /// `true` cuando `latest_version` vino del cache de `config.update`
    /// (chequeo aún fresco), `false` cuando se hizo la request real a
    /// GitHub. El caller lo usa para decidir si vale la pena persistir
    /// `config.toml` de nuevo.
    pub from_cache: bool,
}

/// Chequea si hay una versión más nueva publicada en GitHub, usando el cache
/// de `config.update` (TTL `UpdateConfig::is_check_fresh`) para no pegarle a
/// la API en cada invocación del CLI. Sólo hace la request real cuando el
/// cache está vencido o no hay una versión cacheada; si la request falla, el
/// error se propaga y el caller decide ignorarlo silenciosamente (ver
/// `cli::mod::maybe_notify_update`), igual que
/// `operations::objectives::show` con el fetch de SharePad.
pub async fn check_for_update(config: &mut Config, api_base: &str) -> Result<UpdateCheckResult> {
    let now = Utc::now();
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    let (latest_version, from_cache) = match (
        config.update.is_check_fresh(now),
        config.update.latest_known_version.clone(),
    ) {
        (true, Some(cached)) => (cached, true),
        _ => {
            let release = client::update::fetch_latest_release(api_base)
                .await
                .context("Error al consultar la última versión en GitHub")?;
            let version = release.version().to_string();
            config.update.latest_known_version = Some(version.clone());
            config.update.last_checked_at = Some(now.to_rfc3339());
            (version, false)
        }
    };

    let update_available = is_newer_version(&latest_version, &current_version);

    Ok(UpdateCheckResult {
        current_version,
        latest_version,
        update_available,
        from_cache,
    })
}

/// Compara dos versiones `MAJOR.MINOR.PATCH` (con o sin prefijo `v`,
/// ignorando cualquier sufijo de pre-release/build tras el patch — ej.
/// `0.7.0-rc1` se compara como `0.7.0`). Devuelve `true` sólo si `candidate`
/// es estrictamente mayor que `current`; si cualquiera de las dos no
/// matchea el formato esperado, devuelve `false` en vez de panickear o
/// sugerir una actualización sobre datos que no pudo interpretar.
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let version = version.strip_prefix('v').unwrap_or(version);
    let core = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

/// Resultado de `perform_self_update`.
#[derive(Debug, Clone)]
pub struct SelfUpdateOutcome {
    pub version: String,
    pub binary_path: PathBuf,
    /// `false` cuando ya se estaba en la última versión conocida y
    /// `perform_self_update` cortó antes de descargar nada — evita gastar
    /// ancho de banda y reemplazar el binario por uno idéntico.
    pub updated: bool,
}

/// Descarga el asset de la última release para la plataforma actual y
/// reemplaza el binario en ejecución. El pipeline de release
/// (`.github/workflows/release.yml`) sólo publica `hteam-linux-x86_64`, así
/// que esto falla con un error claro (sin panic) si la plataforma actual no
/// es linux-x86_64, en vez de asumir que el asset siempre existe.
///
/// Antes de descargar nada, compara la versión de la última release contra
/// `env!("CARGO_PKG_VERSION")` con `is_newer_version`; si no hay una
/// versión más nueva, devuelve `updated: false` sin llamar a
/// `client::update::download_asset`.
///
/// El reemplazo escribe el binario descargado en un archivo temporal dentro
/// del mismo directorio que el binario actual y hace `fs::rename` sobre él
/// — en Linux esto es seguro incluso con el binario en ejecución: el
/// proceso corriendo sigue referenciando el inode viejo hasta que termina,
/// y la siguiente vez que se invoque `hteam` el path ya apunta al binario
/// nuevo.
pub async fn perform_self_update(api_base: &str) -> Result<SelfUpdateOutcome> {
    if env::consts::OS != "linux" || env::consts::ARCH != "x86_64" {
        anyhow::bail!(
            "hteam update sólo soporta linux-x86_64 hoy (plataforma actual: {}-{}). \
             El pipeline de release sólo publica el asset '{}'.",
            env::consts::OS,
            env::consts::ARCH,
            client::update::RELEASE_ASSET_NAME
        );
    }

    let release = client::update::fetch_latest_release(api_base)
        .await
        .context("Error al consultar la última versión en GitHub")?;

    let latest_version = release.version().to_string();

    if !is_newer_version(&latest_version, env!("CARGO_PKG_VERSION")) {
        let current_exe =
            env::current_exe().context("No se pudo resolver la ruta del binario actual")?;
        return Ok(SelfUpdateOutcome {
            version: latest_version,
            binary_path: current_exe,
            updated: false,
        });
    }

    let download_url = release.asset_download_url().with_context(|| {
        format!(
            "La release {} no publica el asset '{}'",
            release.tag_name,
            client::update::RELEASE_ASSET_NAME
        )
    })?;

    let bytes = client::update::download_asset(download_url)
        .await
        .context("Error al descargar el binario de la última release")?;

    let current_exe =
        env::current_exe().context("No se pudo resolver la ruta del binario actual")?;
    let dir = current_exe
        .parent()
        .context("El binario actual no tiene directorio padre")?;
    let tmp_path = dir.join(".hteam-update.tmp");

    fs::write(&tmp_path, &bytes).with_context(|| {
        format!(
            "No se pudo escribir el binario descargado en {}",
            tmp_path.display()
        )
    })?;

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&tmp_path)
            .with_context(|| format!("No se pudo leer permisos de {}", tmp_path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp_path, perms)
            .with_context(|| format!("No se pudo marcar {} como ejecutable", tmp_path.display()))?;
    }

    fs::rename(&tmp_path, &current_exe).with_context(|| {
        format!(
            "No se pudo reemplazar el binario en {}",
            current_exe.display()
        )
    })?;

    Ok(SelfUpdateOutcome {
        version: release.version().to_string(),
        binary_path: current_exe,
        updated: true,
    })
}

/// Extrae, del changelog embebido, las secciones de versión estrictamente
/// más nuevas que `last_seen` hasta `current` inclusive. El changelog está
/// ordenado de más nueva a más vieja (así lo genera `cliff.toml`), así que
/// esto sólo recorre desde la sección de `current` hacia abajo hasta
/// encontrar la de `last_seen` (sin incluirla).
///
/// Sin `last_seen` (primera corrida) devuelve sólo la sección de `current`.
/// Si `last_seen` no matchea ninguna sección conocida (config vieja o
/// corrupta), también devuelve sólo la de `current` — nunca vuelca todo el
/// historial sin un ancla confiable. Si `current` tampoco aparece en el
/// changelog (ej. build de desarrollo sin release todavía), devuelve `None`.
pub fn changelog_since(changelog: &str, last_seen: Option<&str>, current: &str) -> Option<String> {
    let sections = parse_changelog_sections(changelog);

    let current_idx = sections
        .iter()
        .position(|(version, _)| version == current)?;

    let end_idx = last_seen
        .and_then(|last| sections.iter().position(|(version, _)| version == last))
        .filter(|&idx| idx > current_idx)
        .unwrap_or(current_idx + 1);

    let picked: Vec<&str> = sections[current_idx..end_idx]
        .iter()
        .map(|(_, body)| body.as_str())
        .collect();

    if picked.is_empty() {
        return None;
    }

    Some(picked.join("\n"))
}

/// Parsea encabezados `## [X.Y.Z] - fecha` (ignora `## [Unreleased]`),
/// devolviendo pares (versión, cuerpo de la sección incluyendo su
/// encabezado), en el mismo orden en que aparecen en el changelog.
fn parse_changelog_sections(changelog: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;

    for line in changelog.lines() {
        if let Some(rest) = line.strip_prefix("## [") {
            if let Some(section) = current.take() {
                sections.push(section);
            }

            match rest.split_once(']') {
                Some((version, _)) if version != "Unreleased" => {
                    current = Some((version.to_string(), format!("{}\n", line)));
                }
                _ => current = None,
            }
            continue;
        }

        if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }

    if let Some(section) = current.take() {
        sections.push(section);
    }

    sections
}

/// Compara `config.update.last_seen_version` contra `env!("CARGO_PKG_VERSION")`.
/// Si difieren (incluida la primera corrida sin valor previo), devuelve el
/// changelog de las versiones nuevas para mostrarlo una única vez, y deja
/// `config.update.last_seen_version` actualizado a la versión actual — el
/// caller decide si persiste (`Config::save()`), igual que
/// `operations::objectives::show` con su propio cache.
pub fn resolve_startup_changelog(config: &mut Config, changelog: &str) -> Option<String> {
    let current_version = env!("CARGO_PKG_VERSION");

    if config.update.last_seen_version.as_deref() == Some(current_version) {
        return None;
    }

    let result = changelog_since(
        changelog,
        config.update.last_seen_version.as_deref(),
        current_version,
    );

    config.update.last_seen_version = Some(current_version.to_string());

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::UpdateConfig;
    use chrono::Duration;

    const SAMPLE_CHANGELOG: &str = "\
# Changelog

## [Unreleased]

### Features
- *(x)* Something not released yet

## [0.7.0] - 2026-09-01

### Features
- *(update)* Add self-update notification

## [0.6.2] - 2026-08-29

### Bug Fixes
- *(client)* Replace useless format! calls with to_string

## [0.6.1] - 2026-08-29

### Documentation
- *(openwiki)* Add generated repository knowledge base
";

    #[test]
    fn is_newer_version_detects_patch_bump() {
        assert!(is_newer_version("0.6.3", "0.6.2"));
    }

    #[test]
    fn is_newer_version_detects_minor_and_major_bumps() {
        assert!(is_newer_version("0.7.0", "0.6.9"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
    }

    #[test]
    fn is_newer_version_is_false_for_equal_versions() {
        assert!(!is_newer_version("0.6.2", "0.6.2"));
    }

    #[test]
    fn is_newer_version_is_false_for_older_candidate() {
        assert!(!is_newer_version("0.6.1", "0.6.2"));
    }

    #[test]
    fn is_newer_version_ignores_v_prefix_and_prerelease_suffix() {
        assert!(is_newer_version("v0.7.0-rc1", "0.6.2"));
        assert!(!is_newer_version("v0.6.2", "0.6.2"));
    }

    #[test]
    fn is_newer_version_is_false_for_unparseable_versions() {
        assert!(!is_newer_version("not-a-version", "0.6.2"));
        assert!(!is_newer_version("0.7.0", "also-not-a-version"));
    }

    #[tokio::test]
    async fn check_for_update_uses_cache_when_fresh() {
        let now = Utc::now();
        let mut config = Config {
            update: UpdateConfig {
                last_checked_at: Some((now - Duration::hours(1)).to_rfc3339()),
                latest_known_version: Some("0.7.0".to_string()),
                last_seen_version: None,
            },
            ..Config::default()
        };

        let result = check_for_update(&mut config, "http://127.0.0.1:1/unreachable")
            .await
            .expect("debería usar el cache");

        assert!(result.from_cache);
        assert_eq!(result.latest_version, "0.7.0");
        assert_eq!(result.current_version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn check_for_update_fetches_when_cache_is_stale() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/releases/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"tag_name":"v99.0.0","assets":[]}"#)
            .create_async()
            .await;

        let mut config = Config {
            update: UpdateConfig {
                last_checked_at: Some((Utc::now() - Duration::hours(7)).to_rfc3339()),
                latest_known_version: Some("0.1.0".to_string()),
                last_seen_version: None,
            },
            ..Config::default()
        };

        let result = check_for_update(&mut config, &server.url())
            .await
            .expect("debería refetch");

        assert!(!result.from_cache);
        assert_eq!(result.latest_version, "99.0.0");
        assert!(result.update_available);
        assert_eq!(
            config.update.latest_known_version.as_deref(),
            Some("99.0.0")
        );
    }

    #[tokio::test]
    async fn check_for_update_propagates_error_on_failure() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/releases/latest")
            .with_status(500)
            .create_async()
            .await;

        let mut config = Config::default();

        let err = check_for_update(&mut config, &server.url())
            .await
            .expect_err("debería fallar");

        assert!(err
            .to_string()
            .contains("Error al consultar la última versión en GitHub"));
    }

    #[tokio::test]
    async fn perform_self_update_skips_download_when_already_up_to_date() {
        let mut server = mockito::Server::new_async().await;
        // La release "más nueva" es la misma que env!("CARGO_PKG_VERSION")
        // y no publica ningún asset — si perform_self_update no cortara
        // antes de descargar, fallaría con "no publica el asset" en vez de
        // devolver Ok(updated: false).
        let body = format!(
            r#"{{"tag_name":"v{}","assets":[]}}"#,
            env!("CARGO_PKG_VERSION")
        );
        let _m = server
            .mock("GET", "/releases/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let outcome = perform_self_update(&server.url())
            .await
            .expect("no debería fallar: no llega a pedir el asset");

        assert!(!outcome.updated);
        assert_eq!(outcome.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn changelog_since_returns_only_current_without_last_seen() {
        let changelog = changelog_since(SAMPLE_CHANGELOG, None, "0.7.0").expect("debería extraer");

        assert!(changelog.contains("## [0.7.0]"));
        assert!(changelog.contains("Add self-update notification"));
        assert!(!changelog.contains("## [0.6.2]"));
    }

    #[test]
    fn changelog_since_returns_range_up_to_last_seen_exclusive() {
        let changelog =
            changelog_since(SAMPLE_CHANGELOG, Some("0.6.1"), "0.7.0").expect("debería extraer");

        assert!(changelog.contains("## [0.7.0]"));
        assert!(changelog.contains("## [0.6.2]"));
        assert!(!changelog.contains("## [0.6.1]"));
    }

    #[test]
    fn changelog_since_falls_back_to_only_current_when_last_seen_is_unknown() {
        let changelog =
            changelog_since(SAMPLE_CHANGELOG, Some("0.0.1"), "0.7.0").expect("debería extraer");

        assert!(changelog.contains("## [0.7.0]"));
        assert!(!changelog.contains("## [0.6.2]"));
    }

    #[test]
    fn changelog_since_returns_none_when_current_version_is_missing() {
        assert_eq!(changelog_since(SAMPLE_CHANGELOG, None, "9.9.9"), None);
    }

    #[test]
    fn changelog_since_ignores_the_unreleased_section() {
        let changelog = changelog_since(SAMPLE_CHANGELOG, None, "0.7.0").expect("debería extraer");

        assert!(!changelog.contains("Something not released yet"));
    }

    #[test]
    fn resolve_startup_changelog_returns_none_on_first_run_when_current_version_is_unreleased() {
        let changelog_text =
            "# Changelog\n\n## [0.0.1] - 2026-01-01\n\n### Features\n- *(x)* vieja\n";
        let mut config = Config::default();

        let changelog = resolve_startup_changelog(&mut config, changelog_text);

        assert_eq!(
            config.update.last_seen_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        // env!("CARGO_PKG_VERSION") no está en este changelog (build de
        // desarrollo sin release todavía), así que no hay nada que mostrar,
        // pero last_seen_version igual se actualiza para no repetir el
        // intento en la próxima corrida.
        assert_eq!(changelog, None);
    }

    #[test]
    fn resolve_startup_changelog_returns_none_when_already_seen() {
        let mut config = Config {
            update: UpdateConfig {
                last_seen_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                ..UpdateConfig::default()
            },
            ..Config::default()
        };

        let changelog = resolve_startup_changelog(&mut config, SAMPLE_CHANGELOG);

        assert_eq!(changelog, None);
    }

    #[test]
    fn resolve_startup_changelog_finds_current_version_section() {
        let changelog_text = format!(
            "# Changelog\n\n## [{}] - 2026-09-01\n\n### Features\n- *(x)* prueba\n",
            env!("CARGO_PKG_VERSION")
        );
        let mut config = Config::default();

        let changelog = resolve_startup_changelog(&mut config, &changelog_text)
            .expect("debería encontrar la sección de la versión actual");

        assert!(changelog.contains("prueba"));
        assert_eq!(
            config.update.last_seen_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }
}
