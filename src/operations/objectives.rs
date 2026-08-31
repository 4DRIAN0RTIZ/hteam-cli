use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use crate::client;
use crate::config::Config;
use crate::models::{WeeklyObjective, WeeklyObjectivesSet, WeeklyTask};

/// Resultado de `show`: el set de objetivos (fresco o de cache) más metadata
/// de presentación que decide `cli::objectives` (hace cuánto se sincronizó).
#[derive(Debug)]
pub struct WeeklyObjectivesView {
    pub set: WeeklyObjectivesSet,
    pub synced_at: DateTime<Utc>,
    pub from_cache: bool,
}

/// Orquesta `hteam objectives show`: decide si sirve el cache local o
/// refresca desde SharePad, y actualiza el cache en memoria sólo si el
/// fetch + parseo son exitosos. No persiste a disco — eso lo hace el caller
/// (`cli::objectives`) con `Config::save()` sólo cuando `from_cache` es
/// `false`, para que esta función quede testeable sin tocar el filesystem
/// real (mismo motivo por el que `boards::switch_board` no se testea acá).
///
/// - Si `force` es `false` y el cache tiene menos de 24h, se devuelve el
///   cache sin tocar la red.
/// - En cualquier otro caso (cache vencido, inexistente, o `force`) se hace
///   el `GET` real a `config.weekly_objectives.source_url`. Si falla el
///   fetch o el parseo, se propaga el error y el cache existente no se
///   toca.
pub async fn show(config: &mut Config, force: bool) -> Result<WeeklyObjectivesView> {
    let now = Utc::now();

    if !force && config.weekly_objectives.is_cache_fresh(now) {
        if let Some(set) = config.weekly_objectives.cached_set() {
            let synced_at = config
                .weekly_objectives
                .last_synced_at
                .as_deref()
                .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(now);

            return Ok(WeeklyObjectivesView {
                set,
                synced_at,
                from_cache: true,
            });
        }
    }

    let markdown =
        client::objectives::fetch_weekly_objectives_markdown(&config.weekly_objectives.source_url)
            .await
            .context("Error al obtener los objetivos semanales de SharePad")?;

    let set = parse_markdown(&markdown)?;

    config.weekly_objectives.last_synced_at = Some(now.to_rfc3339());
    config.weekly_objectives.cached_title = Some(set.title.clone());
    config.weekly_objectives.cached_objectives = set.objectives.clone();

    Ok(WeeklyObjectivesView {
        set,
        synced_at: now,
        from_cache: false,
    })
}

/// Parsea el markdown de un notebook de SharePad a un `WeeklyObjectivesSet`:
/// el primer `# titulo` (H1) es el título del set, cada `## nombre` (H2) es
/// un objetivo, y los `- [ ] tarea` / `- [x] tarea` debajo de un H2 son sus
/// tareas asociadas (las tareas fuera de cualquier H2 se ignoran).
pub fn parse_markdown(markdown: &str) -> Result<WeeklyObjectivesSet> {
    let mut title: Option<String> = None;
    let mut objectives: Vec<WeeklyObjective> = Vec::new();

    for raw_line in markdown.lines() {
        let line = raw_line.trim();

        if let Some(rest) = line.strip_prefix("## ") {
            objectives.push(WeeklyObjective {
                name: rest.trim().to_string(),
                tasks: Vec::new(),
            });
        } else if let Some(rest) = line.strip_prefix("# ") {
            if title.is_none() {
                title = Some(rest.trim().to_string());
            }
        } else if let Some(task) = parse_checklist_item(line) {
            if let Some(current) = objectives.last_mut() {
                current.tasks.push(task);
            }
        }
    }

    let title = title.context(
        "No se encontró un título (# titulo) en el markdown de objetivos semanales de SharePad",
    )?;

    if objectives.is_empty() {
        anyhow::bail!(
            "No se encontró ningún objetivo (## nombre) en el markdown de objetivos semanales de SharePad"
        );
    }

    Ok(WeeklyObjectivesSet { title, objectives })
}

/// Parsea una línea `- [ ] texto` / `- [x] texto` (checklist de GitHub
/// Flavored Markdown). Devuelve `None` si la línea no es un checklist item.
fn parse_checklist_item(line: &str) -> Option<WeeklyTask> {
    let rest = line.strip_prefix("- [")?;
    let (mark, description) = rest.split_once(']')?;
    let done = mark.trim().eq_ignore_ascii_case("x");

    Some(WeeklyTask {
        description: description.trim().to_string(),
        done,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WeeklyObjectivesConfig;
    use chrono::Duration;

    const VALID_MARKDOWN: &str = "# Objetivos semanales\n\n## Demo CCL\n- [ ] Tarea 1 xd\n- [x] Tarea 2 lista\n\n## Otro objetivo\n- [ ] Tarea 3\n";

    #[test]
    fn parse_markdown_extracts_title_objectives_and_tasks() {
        let set = parse_markdown(VALID_MARKDOWN).expect("markdown válido");

        assert_eq!(set.title, "Objetivos semanales");
        assert_eq!(set.objectives.len(), 2);

        assert_eq!(set.objectives[0].name, "Demo CCL");
        assert_eq!(set.objectives[0].tasks.len(), 2);
        assert_eq!(set.objectives[0].tasks[0].description, "Tarea 1 xd");
        assert!(!set.objectives[0].tasks[0].done);
        assert_eq!(set.objectives[0].tasks[1].description, "Tarea 2 lista");
        assert!(set.objectives[0].tasks[1].done);

        assert_eq!(set.objectives[1].name, "Otro objetivo");
        assert_eq!(set.objectives[1].tasks.len(), 1);
    }

    #[test]
    fn parse_markdown_ignores_tasks_before_any_objective() {
        let markdown = "# Solo titulo\n- [ ] tarea huerfana\n## Objetivo real\n- [ ] tarea valida";

        let set = parse_markdown(markdown).expect("markdown válido");

        assert_eq!(set.objectives.len(), 1);
        assert_eq!(set.objectives[0].tasks.len(), 1);
        assert_eq!(set.objectives[0].tasks[0].description, "tarea valida");
    }

    #[test]
    fn parse_markdown_fails_without_title() {
        let markdown = "## Objetivo sin titulo\n- [ ] tarea";

        let err = parse_markdown(markdown).expect_err("debería fallar sin título");

        assert!(err.to_string().contains("título"));
    }

    #[test]
    fn parse_markdown_fails_without_objectives() {
        let markdown = "# Solo titulo, sin objetivos";

        let err = parse_markdown(markdown).expect_err("debería fallar sin objetivos");

        assert!(err.to_string().contains("objetivo"));
    }

    #[tokio::test]
    async fn show_returns_cache_when_fresh_and_not_forced() {
        let now = Utc::now();
        let mut config = Config {
            weekly_objectives: WeeklyObjectivesConfig {
                source_url: "http://127.0.0.1:1/unreachable".to_string(),
                last_synced_at: Some((now - Duration::hours(1)).to_rfc3339()),
                cached_title: Some("Objetivos semanales".to_string()),
                cached_objectives: vec![WeeklyObjective {
                    name: "Demo CCL".to_string(),
                    tasks: vec![],
                }],
            },
            ..Config::default()
        };

        let view = show(&mut config, false).await.expect("debería usar cache");

        assert!(view.from_cache);
        assert_eq!(view.set.title, "Objetivos semanales");
    }

    #[tokio::test]
    async fn show_fetches_when_forced_even_with_fresh_cache() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/n/okr-semanales")
            .with_status(200)
            .with_body(
                r##"<html><body><script>self.__next_f.push([1,"3:{\"pages\":[{\"content\":\"# Fresco\\n## Obj\\n- [ ] t\"}]}"])</script></body></html>"##,
            )
            .create_async()
            .await;

        let now = Utc::now();
        let mut config = Config {
            weekly_objectives: WeeklyObjectivesConfig {
                source_url: format!("{}/n/okr-semanales", server.url()),
                last_synced_at: Some((now - Duration::hours(1)).to_rfc3339()),
                cached_title: Some("Viejo".to_string()),
                cached_objectives: vec![],
            },
            ..Config::default()
        };

        let view = show(&mut config, true).await.expect("debería refetch");

        assert!(!view.from_cache);
        assert_eq!(view.set.title, "Fresco");
        assert_eq!(
            config.weekly_objectives.cached_title.as_deref(),
            Some("Fresco")
        );
    }

    #[tokio::test]
    async fn show_does_not_touch_cache_when_fetch_fails() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/n/okr-semanales")
            .with_status(500)
            .create_async()
            .await;

        let mut config = Config {
            weekly_objectives: WeeklyObjectivesConfig {
                source_url: format!("{}/n/okr-semanales", server.url()),
                last_synced_at: Some((Utc::now() - Duration::hours(48)).to_rfc3339()),
                cached_title: Some("Anterior".to_string()),
                cached_objectives: vec![],
            },
            ..Config::default()
        };

        let err = show(&mut config, false).await.expect_err("debería fallar");

        assert!(err
            .to_string()
            .contains("Error al obtener los objetivos semanales de SharePad"));
        assert_eq!(
            config.weekly_objectives.cached_title.as_deref(),
            Some("Anterior")
        );
    }
}
