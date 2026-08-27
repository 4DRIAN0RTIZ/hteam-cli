# Convenciones de código

> Homogeneidad extrema. La IA predice mejor cuando el repositorio se parece
> a sí mismo en todas partes.

## Estilo Rust

- **Edición:** 2021, tal como está en `Cargo.toml`.
- **Formato:** `cargo fmt` (rustfmt default, sin config custom). Nunca
  entregues código sin pasarlo por `cargo fmt`.
- **Lints:** `cargo clippy` debe salir sin warnings. Si un warning es un
  falso positivo genuino, se permite `#[allow(...)]` puntual con un
  comentario que explique por qué (no un `allow` global).
- **Errores:** `anyhow::Result` en el borde de la aplicación (CLI, MCP,
  TUI); `thiserror` para tipos de error de dominio cuando el caller
  necesita matchear variantes. Usa `.context("mensaje en español, específico")`
  igual que ya se hace en `client/mod.rs`.
- **Async:** `tokio` con `#[tokio::main]` / `async fn`. No mezcles bloqueo
  sincrónico con IO dentro de funciones `async`.

## Nombres

| Tipo                  | Convención     | Ejemplo                  |
|------------------------|----------------|---------------------------|
| Módulos                | `snake_case`   | `daily_work.rs`           |
| Structs / Enums        | `PascalCase`   | `HteamClient`, `CardDetail` |
| Funciones / variables  | `snake_case`   | `resolve_board_number`    |
| Constantes             | `UPPER_SNAKE`  | `BASE_URL`, `COMMENT_DATE_FORMAT` |
| Privadas               | prefijo no aplica en Rust (usa `pub(crate)` / sin `pub`) | `fn truncate_for_error` |

## Estructura de módulo

- Un archivo por dominio en `operations/`, `models/`, `cli/` — mismo patrón
  que ya existe (`operations/cards.rs`, `operations/boards.rs`, ...).
- Doc-comment (`//!`) al inicio de cada `mod.rs` que explica el rol del
  módulo, igual que `src/operations/mod.rs`.
- `pub use` en el `mod.rs` del módulo para reexportar lo que consumen otras
  capas (ver `pub use session::{resolve_board_number, Session};`).

## Tests

- Un archivo de test por módulo relevante: `#[cfg(test)] mod tests` dentro
  del propio archivo para unit tests, o `tests/integration/<módulo>.rs` para
  tests de integración contra el CLI/MCP real.
- `mockito` (ya en `dev-dependencies`) para mockear las respuestas HTTP de
  `hteam.mx` en tests de `operations/` y `client/`. No hagas requests reales
  en tests.
- Nombres de test descriptivos: `test_resolve_board_number_falls_back_to_default`.

## Manejo de errores

El CLI captura errores de dominio, imprime mensaje claro a `stderr` (o vía
`colored` si ya es el patrón en ese módulo) y sale con código != 0. Nunca
propaga un panic ni un stack trace crudo al usuario.

## Comentarios

Por defecto **no** se escriben. Solo se permiten cuando explican un *por qué*
no obvio (invariante sutil, workaround documentado — como el de
`truncate_for_error` en `client/mod.rs`, que explica por qué corta por char
y no por byte). Los nombres deben hacer el resto.
