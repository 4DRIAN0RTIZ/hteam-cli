# Arquitectura — Qué significa "hacer un buen trabajo"

> Este documento define el estándar de calidad. Los agentes revisores
> evalúan código contra este archivo. Si no está aquí, no es un requisito.

## Capas (ya existentes, no inventar nuevas sin justificar en `feature_list.json`)

```
usuario ─→ cli/        (clap: parseo de argumentos, subcomandos)
           repl/        (REPL interactivo, historial persistente)
           tui/         (ratatui: UI interactiva de terminal)
           mcp/         (servidor MCP sobre stdio: expone operations:: como tools de IA)
                │
                └─→ operations/   (lógica de negocio pura: boards, cards, checkin,
                                    comments, daily_work, projects, reminders,
                                    session, users, working)
                         │
                         └─→ client/   (HteamClient: HTTP real contra hteam.mx,
                                         manejo de cookies sessionid/csrftoken)
                         └─→ config/   (persistencia de config en disco: boards,
                                         board activo, credenciales)
                         └─→ models/   (structs de datos: Card, Board, Comment, etc.)
```

**Regla de oro** (ya documentada en `src/operations/mod.rs`): toda función de
`operations/` recibe `&HteamClient` (y `&mut Config` si persiste algo) y
devuelve datos de dominio puros — nada de `println!`, nada de `ratatui`,
nada de formateo JSON. Cada superficie de entrega (`cli`, `tui`, `mcp`, `repl`)
es un adaptador delgado que llama a `operations/` y decide cómo presentar
el resultado.

## Qué NO hacer

- No metas lógica de negocio en `cli/`, `tui/`, `mcp/` o `repl/`. Si una
  función empieza a decidir "qué hacer", no "qué mostrar", pertenece a
  `operations/`.
- No dupliques llamadas HTTP directas fuera de `client/mod.rs`. Todo el
  tráfico a `hteam.mx` pasa por `HteamClient`.
- No agregues una capa nueva (repositorios, ORMs, un segundo cliente HTTP)
  sin que haya una razón concreta documentada en `feature_list.json`.
- No mezcles la resolución de `board_number` (ver `operations::session`) con
  lógica específica de una feature — reusá `resolve_board_number`.
- No dupliques en `feature_list.json` lo que ya está en `CHANGELOG.md`. El
  changelog es la fuente de verdad de lo ya hecho; el backlog es solo lo
  que falta.
