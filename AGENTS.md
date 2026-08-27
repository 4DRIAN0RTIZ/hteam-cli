# AGENTS.md — Mapa de navegación para agentes de IA

> Este archivo es el **punto de entrada** para cualquier agente que trabaje en
> este repositorio. NO es una biblia de reglas: es un **mapa**. Lee solo lo
> que necesites cuando lo necesites (divulgación progresiva).

---

## 1. Antes de empezar (obligatorio)

1. Ejecuta `./init.sh` y verifica que termina sin errores. Si falla, **para**
   y resuelve el entorno antes de tocar código.
2. Lee `progress/current.md` para entender en qué estado quedó la última sesión.
3. Lee `feature_list.json`. Si hay una feature `pending`, elige **una**. Si
   está vacío, es un proyecto en régimen de mantenimiento normal — segui las
   instrucciones puntuales que te dé el usuario, aplicando igual las reglas
   de `docs/` y `CHECKPOINTS.md`.
4. Cada que vayas a implementar algo, debes de asegurarte de que no hay cambios remotos sin bajar en las ramas main y dev.
5. Crear rama siguiendo el formato conventional commits (feat/fix/etc) segun la feature partiendo de la rama `dev`

## 2. Mapa del repositorio

| Archivo / carpeta          | Qué contiene                                                | Cuándo leerlo |
|-----------------------------|--------------------------------------------------------------|---------------|
| `feature_list.json`         | Backlog de trabajo futuro (pending / in_progress / done / blocked). Vacío = no hay features grandes en curso. | Siempre, al empezar |
| `progress/current.md`       | Estado de la sesión activa                                    | Siempre, al empezar |
| `progress/history.md`       | Bitácora append-only de sesiones anteriores                   | Si necesitas contexto histórico |
| `docs/architecture.md`      | Capas reales del CLI y qué significa "hacer un buen trabajo"  | Antes de implementar |
| `docs/conventions.md`       | Estilo Rust, nombres, manejo de errores                        | Antes de escribir código |
| `docs/verification.md`      | Cómo verificar que el trabajo funciona (cargo test/clippy/fmt) | Antes de declarar una tarea como `done` |
| `CHECKPOINTS.md`            | Criterios objetivos de "estado final correcto"                 | Para auto-evaluarte |
| `.claude/agents/`           | Definiciones de subagentes (líder, implementador, revisor, archivista) | Si orquestas trabajo |
| `progress/archive/`         | Reportes de sesiones ya cerradas por el archivista (por feature) | Para auditoría histórica |
| `src/`                      | Código de la aplicación (cli/ → operations/ → client/, models/, config/, mcp/, tui/, repl/) | Para implementar |
| `tests/`                    | Tests automáticos (unit en `src/`, integración en `tests/`)    | Para verificar |
| `CHANGELOG.md`              | Historial real de releases (generado con git-cliff)            | Para ver qué ya se hizo, no lo dupliques en `feature_list.json` |

## 3. Reglas duras (no negociables)

- **Una sola feature a la vez** cuando trabajes desde `feature_list.json`.
  No mezcles cambios de varias tareas en la misma sesión.
- **No declares una tarea `done` sin pruebas verdes.** Ejecuta `./init.sh` y
  asegúrate de que compila, pasa clippy sin warnings, y los tests pasan.
- **Todo módulo que toques necesita test propio**, aunque el resto del
  repo hoy no lo tenga (ver `docs/verification.md` — no se exige retro-cubrir
  módulos que no tocaste).
- **Documenta lo que haces** en `progress/current.md` mientras trabajas, no
  al final.
- **Deja el repositorio limpio** antes de cerrar la sesión (ver §5).
- **Si no sabes algo, busca en `docs/`** antes de inventarlo.

## 4. Cómo elegir una tarea

```
1. Abre feature_list.json
2. Filtra por status == "pending"
3. Coge la de menor "id"
4. Cambia su status a "in_progress" y guarda
5. Anota en progress/current.md: feature, hora de inicio, plan breve
```

Si `feature_list.json` no tiene features `pending` (backlog vacío), trabajás
directo sobre lo que pida el usuario, pero segui documentando en
`progress/current.md` igual.

## 5. Cierre de sesión (lifecycle)

Antes de terminar:

1. Ejecuta `./init.sh` — todo verde (compila, clippy limpio, tests OK).
2. Si la tarea está acabada: marca `status: "done"` en `feature_list.json`
   (si venís de ahí).
3. Mueve el resumen de `progress/current.md` al final de `progress/history.md`.
4. Vacía `progress/current.md` dejando solo la plantilla.
5. No dejes archivos temporales, ni `println!`/`dbg!` de debug, ni TODOs sin
   contexto.

## 6. Si te bloqueas

- Relee la sección relevante de `docs/`.
- Si la herramienta no hace lo que esperas, **no inventes un workaround**:
  documenta el bloqueo en `progress/current.md` y para la sesión.
