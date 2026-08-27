# Historia — Bitácora de sesiones cerradas

> Append-only. Cada sesión cerrada agrega una entrada abajo, nunca se edita
> una entrada anterior.

## 2026-08-26 — Bootstrap del harness

- Se armó el harness completo (AGENTS.md, docs/, CHECKPOINTS.md,
  feature_list.json, progress/, .claude/agents/, .claude/settings.json)
  tomando como referencia `~/harness-sample/` y adaptándolo a un proyecto
  Rust real y ya maduro (v0.6.0), en vez de un ejercicio didáctico
  greenfield.
- Hallazgo clave: el repo tenía 0 tests automáticos (`tests/integration` y
  `tests/mocks` vacíos, `mockito` sin usar). Se decidió NO exigir
  retro-cobertura de los ~20 módulos existentes; la regla de tests aplica
  solo a módulos tocados de acá en adelante.
- Se descartó completar `feature_list.json` con lo ya hecho — `CHANGELOG.md`
  (generado con git-cliff) ya cumple ese rol; el backlog arranca vacío.

## 2026-08-26 — Mantenimiento: formateo rustfmt


- **Feature en curso:** mantenimiento: formateo rustfmt
- **Inicio:** 2026-08-26
- **Agente:** pi

## Plan

- Verificar estado inicial con `./init.sh`.
- Ejecutar `cargo fmt` para aplicar rustfmt.
- Comprobar que no queden diferencias de formato con `cargo fmt -- --check`.

## Bitácora

- `./init.sh` falló porque había archivos sin formatear; también reportó errores previos de clippy no relacionados con rustfmt.
- Ejecutado `cargo fmt` y verificado con `cargo fmt -- --check` sin salida/errores.
- `git diff --stat` muestra cambios de formato en archivos Rust existentes.
- Re-ejecutado `./init.sh`: `cargo fmt --check` queda OK y los tests pasan; el log de clippy imprime errores existentes, aunque el script termina marcando el entorno como listo.

## Próximo paso

Si se cierra la sesión, decidir si corregir los errores de clippy existentes o solo commitear el formateo.

## 2026-08-26 — Feature #1: cargo_fmt_codebase

- **Inicio:** 2026-08-26
- **Agente:** pi

## Plan

- Normalizar el estado de git según AGENTS: traer `main`/`dev` y trabajar en rama desde `dev`.
- Revisar los errores actuales de clippy que impiden cerrar la feature.
- Corregir solo lo necesario para que `cargo fmt`, `cargo clippy` y `cargo test` queden verdes.
- Ejecutar `./init.sh` final antes de marcar la feature como `done`.

## Bitácora

- `./init.sh` inicial: formato y tests pasan; clippy emite errores con `-D warnings` aunque el script imprime OK por un bug del arnés.
- `origin/main` está al día; `origin/dev` tenía 3 commits remotos pendientes.
- Se guardó el WIP en stash, se actualizó `dev` con fast-forward hasta `origin/dev` y se creó `feature/cargo-fmt-codebase`; luego se reaplicó el WIP.
- Corregidos warnings de clippy: import redundante, dead code explícitamente esperado en campos de compatibilidad API, argumentos de `update_card_full`, firmas `&String`/`&Vec` y match colapsable del TUI.
- `cargo fmt` y `cargo clippy --all-targets --all-features -- -D warnings` pasan.
- `cargo test` pasa.
- `./init.sh` final termina con `[OK] Entorno listo`.
- Feature #1 marcada como `done` en `feature_list.json`.

## Próximo paso

Revisar el diff y preparar commit/PR de `feature/cargo-fmt-codebase`.


## 2026-08-27 — Feature #2: unit_tests_operations

- **Feature en curso:** #2 `unit_tests_operations` — test: agregar unit tests a `src/operations/`
- **Inicio:** 2026-08-27
- **Agente:** Pi

## Plan

- Verificar entorno y estado de ramas `main`/`dev` antes de tocar código.
- Revisar `src/operations/` para elegir una primera tanda coherente de tests.
- Agregar unit tests con `mockito` sin requests reales.
- Ejecutar `cargo fmt`, `cargo test`, clippy/`./init.sh` y documentar resultados.

## Bitácora

- Ejecutado `./init.sh`: entorno inicial verde, pero todavía sin tests (`0 passed`).
- Ejecutado `git fetch --all --prune`; `dev` está al día con `origin/dev`. Se creó la rama `test/unit-tests-operations` desde `dev`.
- Se preservó el cambio existente en `AGENTS.md` y no se tocó como parte de la feature.
- `feature_list.json` actualizado: feature #2 en `in_progress`.

## Próximo paso

Inspeccionar los módulos de `src/operations/` y empezar por tests de funciones con entradas/salidas acotadas o HTTP mockeable.
- Inspeccionados `src/operations/*.rs`; los wrappers HTTP dependen de URLs constantes en `HteamClient`, así que se preparó un constructor `#[cfg(test)]` para apuntar a `mockito` sin tocar producción.
- Agregada infraestructura de test `HteamClient::new_for_test` para redirigir API/site a `mockito` solo en `#[cfg(test)]`.
- Agregados 27 unit tests cubriendo wrappers y lógica de `operations/` (boards, cards, checkin, comments, daily_work, projects, reminders, session, users, working) más cobertura mínima del helper del cliente tocado.
- Ejecutado `cargo fmt && cargo test`: 27 tests en verde.
- Ejecutado `cargo clippy --all-targets --all-features -- -D warnings`: verde.
- Ejecutado `./init.sh`: verde; `cargo fmt --check`, clippy y 27 tests pasan.
- Feature #2 marcada como `done` en `feature_list.json`.
