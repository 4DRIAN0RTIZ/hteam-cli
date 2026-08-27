# CHECKPOINTS — Evaluación del estado final

> En sistemas multi-agente no se evalúa el camino, se evalúa el destino.
> Estos son los checkpoints objetivos que un juez (humano o IA) puede usar
> para decidir si el proyecto está sano.

## C1 — El arnés está completo

- [ ] Existen los 4 archivos base: `AGENTS.md`, `init.sh`, `feature_list.json`,
      `progress/current.md`.
- [ ] Existen los 3 docs: `docs/architecture.md`, `docs/conventions.md`,
      `docs/verification.md`.
- [ ] `./init.sh` termina con exit code 0.

## C2 — El estado es coherente

- [ ] Como mucho una feature en `in_progress` en `feature_list.json`.
- [ ] Toda feature `done` (agregada después del bootstrap del harness) tiene
      tests asociados que pasan.
- [ ] `progress/current.md` está vacío o describe la sesión activa
      (no contiene basura de sesiones anteriores).

## C3 — El código respeta la arquitectura

- [ ] La lógica de negocio vive en `operations/`, no en `cli/`, `tui/`,
      `mcp/` ni `repl/` (ver `docs/architecture.md`).
- [ ] No hay llamadas HTTP directas fuera de `client/mod.rs`.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` no tira
      warnings.
- [ ] No hay `println!`/`dbg!` sueltos de debug, ni TODOs sin contexto.

## C4 — La verificación es real

- [ ] Todo módulo tocado en la sesión tiene al menos un test que pasa
      (no se exige retroactividad sobre módulos no tocados).
- [ ] Los tests de `operations/`/`client/` mockean HTTP con `mockito`, no
      pegan contra `hteam.mx` real.
- [ ] `cargo test` sale verde.
- [ ] `cargo fmt --check` sale verde.

## C5 — La sesión se cerró bien (responsabilidad del archivista)

- [ ] No hay archivos sin trackear sospechosos fuera del `.gitignore`.
- [ ] `progress/history.md` tiene una entrada por la última sesión.
- [ ] La última feature trabajada (si venía de `feature_list.json`) está
      reflejada en su estado correcto (`done`).
- [ ] Los reportes de la sesión (`progress/review.md`, `progress/impl_*.md`,
      etc.) fueron movidos a `progress/archive/<id>-<name>/`, no borrados.

---

**Cómo usar este archivo:** el agente revisor (`.claude/agents/reviewer.md`)
recorre C1-C4 y marca `[x]`/`[ ]` antes de dar veredicto. El archivista
(`.claude/agents/archiver.md`) recorre C5 al cerrar, y solo actúa si el
reviewer ya dio `APPROVED`.
