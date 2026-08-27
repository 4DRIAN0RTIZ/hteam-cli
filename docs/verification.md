# Verificación — Cómo demostrar que el trabajo funciona

> Regla de oro: **el agente no dice "funciona", lo demuestra**.
> Toda feature termina con evidencia ejecutable, no con afirmaciones.

## Punto de partida real de este repo

Hoy (bootstrap del harness) el repo tiene **cero tests automáticos**.
`tests/integration/` y `tests/mocks/` existen pero están vacíos. Esto NO se
exige retroactivamente: no hay que cubrir los ~20 módulos ya existentes de
una sola vez. La regla es hacia adelante:

> **Todo módulo que crees o modifiques a partir de ahora necesita su test.**
> Un módulo sin tocar puede seguir sin tests hasta que alguien lo toque.

## Niveles de verificación

### Nivel 1 — Compilación + lints (obligatorio, siempre)

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Si `cargo fmt --check` falla, corré `cargo fmt` antes de seguir. Si clippy
tira warnings, no se ignoran con `#[allow]` salvo caso puntual justificado
(ver `docs/conventions.md`).

### Nivel 2 — Unit tests del módulo tocado (obligatorio si tocaste `operations/`, `client/`, `models/` o `config/`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_board_number_falls_back_to_default() {
        // ...
    }
}
```

Para `operations/` y `client/`, mockeá el HTTP con `mockito` (ya está en
`dev-dependencies`, no agregues otra librería de mocking):

```rust
let mut server = mockito::Server::new();
let _m = server.mock("GET", "/api/boards")
    .with_status(200)
    .with_body(r#"{"boards": []}"#)
    .create();
```

### Nivel 3 — Test de integración del CLI/MCP (obligatorio para features que agregan un subcomando o una tool de MCP)

```bash
cargo test --test integration
```

Los tests de `tests/integration/` invocan el binario compilado o las
funciones públicas de `operations/` end-to-end, contra un servidor mockeado
(no contra `hteam.mx` real).

### Nivel 4 — Smoke test manual (opcional pero recomendado para TUI/REPL)

Los cambios de `tui/` o `repl/` no son fácilmente testeables por unit test.
Ejecutá el binario manualmente y describí en `progress/current.md` qué
probaste y qué viste, en vez de solo afirmar "funciona".

## Anti-patrones (no hacer)

- ❌ "Agregué el comando, debería funcionar." → falta test ejecutable.
- ❌ Test que solo verifica que la función no panickea. → tiene que
  comprobar el resultado concreto.
- ❌ Request real a `hteam.mx` en un test. → usa `mockito`.
- ❌ Marcar la feature como `done` sin pasar `./init.sh`.
- ❌ Agregar una dependencia nueva de testing cuando `mockito` ya cubre
  el caso.

## Verificación final antes de cerrar

```bash
./init.sh           # debe terminar con [OK] Entorno listo
```

Si `./init.sh` está rojo, **no** marques nada como `done`. Anota el bloqueo
en `progress/current.md` con estado `blocked` en `feature_list.json` (si
aplica).
