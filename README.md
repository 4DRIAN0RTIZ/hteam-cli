# hteam

CLI en Rust para gestionar boards de Hteam, con REPL, TUI tipo kanban y servidor MCP sobre stdio.

## Características

- Autenticación con cookies de sesión (`sessionid` + `csrftoken`)
- REPL interactivo con historial (`~/.hteam_history`)
- Servidor MCP sobre stdio: 30 tools
- Multi-board, temas del TUI y self-update
- `--json` en la mayoría de los comandos (ver abajo: no es universal)

## Instalación

```bash
git clone https://github.com/4DRIAN0RTIZ/hteam-cli.git
cd hteam-cli
cargo build --release
# Opcional: instalar globalmente
cargo install --path .
```

## Autenticación

```bash
hteam login
hteam login --board 483   # opcional: fijar board por defecto
hteam doctor              # config + sesión + board (no muta nada)
hteam doctor --json
```

`login` pide `sessionid` y `csrftoken`. Valida la sesión listando tus boards (sin board hardcodeado). Después resuelve el board activo: `--board` si lo pasás y es visible, el de config si sigue en el listado, el único board si solo hay uno, o te deja en `hteam board switch` si hay varios. No guarda la config si la sesión es inválida.

## Comandos

### Board

```bash
hteam board list                          # Boards en config.toml
hteam board switch 483 --name "Care"      # Cambiar board activo
hteam board current                       # Board actual
```

`board list` honra `--json`. `board switch` y `board current` no: son mensajes de texto.

### Listas y cards

```bash
hteam lists                               # Listas del board
hteam open                                # Cards abiertos (lista 1)
hteam closed                              # Cards cerrados (lista 2)
hteam cards list 1                        # Cards de una lista
hteam cards search -q login --priority 1  # Texto, responsable, prioridad, labels
hteam cards detail 12345
hteam cards labels 12345                  # Labels disponibles (solo lectura)
hteam cards comments 12345
hteam cards link 12345                    # Copia el URL al portapapeles
hteam cards link 12345 --no-copy          # Solo imprime el URL
```

### Crear y modificar cards

```bash
hteam cards create --name "Fix login bug"
hteam cards create --name "Fix login bug" --list-id 3

hteam cards move 12345 --to 2
hteam cards move 12345 --to 2 --from 1

# close/reopen usan la familia operation o project (se infiere del title_url)
hteam cards close 12345
hteam cards close 12345 --family operation
hteam cards reopen 12345
hteam cards reopen 6005 --family project

hteam cards dates 12345 --start 2026-10-14
hteam cards dates 12345 --due 2026-10-20
hteam cards time 12345 --estimate "1h 30m"
hteam cards time 12345 --spent 45m
hteam cards time 12345 --spent +30m          # + suma al acumulado

hteam cards update-desc 12345 --description "Nueva descripción"
hteam cards update 12345 --name "Nuevo nombre"
hteam cards update 12345 --priority 4        # 1=muy baja … 5=muy alta (form Hteam)
hteam cards update 12345 --responsible usuario

hteam cards comment 12345 --text "Trabajando en esto"
hteam cards remind 12345
```

### Seguimientos (follow-up)

```bash
hteam cards follow-up 12345
hteam cards follow-up-complete 88            # ID del seguimiento, no del card
hteam cards follow-up-cancel 88
```

El id de seguimiento sale de `follow-up`, no del número visible del card.

### Working On It

```bash
hteam working list
hteam working start 12345                    # ID del card
hteam working stop 8954                      # ID del registro, no del card
```

### Proyectos

```bash
hteam project list                           # Por defecto status Execution
hteam project list --status All
hteam project milestones 99
hteam project tasks 99
hteam project link 99
hteam project link 99 --no-copy
```

`--board` global no cambia el proyecto: estos comandos van por `project_id`.

### Objetivos semanales

```bash
hteam objectives show
hteam objectives show --force                # Ignora el cache de 24h
hteam objectives show --verbose              # Dice si vino de cache o de SharePad
```

No usa la sesión de hteam.mx: scrape de SharePad + cache en `config.toml`. `--board` no aplica.

### Temas del TUI

```bash
hteam theme list                             # Presets + custom en ~/.config/hteam/themes/
hteam theme set solarized                    # default, solarized, high-contrast, o un .toml
```

Un custom inválido falla en `theme set`, no al abrir el TUI. `--board` no aplica.

### Update

```bash
hteam update                                 # linux-x86_64; reemplaza el binario en ejecución
hteam update --json
```

En otras plataformas falla con un error claro. `--board` no aplica.

### Usuarios y misc

```bash
hteam users "juan"
hteam reminders
hteam checkin
hteam daily-work
hteam daily-work --user juan --type time --range this_week
```

### TUI, REPL, MCP

```bash
hteam tui                                    # Kanban; honra --board
hteam interactive                            # REPL
hteam mcp                                    # MCP sobre stdio (JSON-RPC; --json no aplica)
```

### Autocompletado de shell

Los scripts versionados viven en `completions/` (bash, zsh, fish). Regenerarlos cuando cambie clap:

```bash
hteam completions bash > completions/hteam.bash
hteam completions zsh  > completions/_hteam
hteam completions fish > completions/hteam.fish
# también: elvish, powershell
```

Instalar, por ejemplo:

```bash
# zsh: copiar a un directorio de $fpath
cp completions/_hteam ~/.zsh/completions/_hteam
# bash
source completions/hteam.bash
# fish
cp completions/hteam.fish ~/.config/fish/completions/hteam.fish
```

`cargo test` incluye un snapshot: si clap cambia y no regenerás esos archivos, el test falla.

## MCP

30 tools, una por `#[tool]` en `src/mcp/mod.rs`:

| Herramienta | Descripción |
|---|---|
| `lists` | Listas del board activo |
| `cards_list` | Cards de una lista |
| `cards_search` | Buscar y filtrar cards |
| `cards_open` | Cards abiertos |
| `cards_closed` | Cards cerrados |
| `card_detail` | Detalle de un card |
| `card_labels` | Labels disponibles |
| `card_create` | Crear card |
| `card_move` | Mover card |
| `card_close` | Cerrar task |
| `card_reopen` | Reabrir task |
| `card_dates` | Editar start/due |
| `card_time` | Estimado / tiempo gastado |
| `card_update_desc` | Actualizar descripción |
| `card_update` | Nombre, descripción, prioridad, responsable |
| `card_comments` | Comentarios |
| `card_comment` | Agregar comentario |
| `card_remind` | Crear recordatorio |
| `card_follow_up` | Seguimiento agendado |
| `card_follow_up_complete` | Completar seguimiento |
| `card_follow_up_cancel` | Cancelar seguimiento |
| `working_list` | Working On It |
| `working_start` | Empezar a trabajar |
| `working_stop` | Dejar de trabajar |
| `project_milestones` | Milestones de un proyecto |
| `project_tasks` | Tasks de un proyecto |
| `users_search` | Buscar usuarios |
| `reminders` | Recordatorios pendientes |
| `check_in` | Check in |
| `daily_work` | Historial de trabajo diario |

Claude Desktop:

```json
{
  "mcpServers": {
    "hteam": {
      "command": "/ruta/absoluta/al/binario/hteam",
      "args": ["mcp"]
    }
  }
}
```

Claude Code:

```json
{
  "mcpServers": {
    "hteam": {
      "command": "hteam",
      "args": ["mcp"],
      "type": "stdio"
    }
  }
}
```

## REPL

```bash
hteam interactive
```

Prompt: `hteam> `. Un subconjunto del CLI (no cubre theme, update, follow-up, search, objectives, doctor):

| Comando | Descripción |
|---|---|
| `lists`, `ls` | Listar listas |
| `open`, `o` | Cards abiertos |
| `closed`, `c` | Cards cerrados |
| `cards <list_id>` | Cards de una lista |
| `create <nombre>`, `mk <nombre>` | Crear card |
| `move <id> --to <n>`, `mv` | Mover card |
| `close <id>` / `reopen <id>` | Cerrar / reabrir |
| `dates <id> --start/--due` | Fechas |
| `time <id> --estimate/--spent` | Tiempo |
| `detail <id>`, `show <id>` | Detalle |
| `comment <id> <texto>`, `co` | Comentar |
| `working list`, `w list` | Working On It |
| `working start <card_id>` | Empezar a trabajar |
| `working stop <working_id>` | Dejar de trabajar |
| `board list` / `switch` / `current` | Boards |
| `config show` | Configuración |
| `! <comando>` | Shell |
| `clear`, `cls` | Limpiar |
| `help`, `h`, `?` | Ayuda |
| `exit`, `quit`, `q` | Salir |

## `--json` y `--board`

Clap los marca como flags globales, así que aparecen en `hteam <cmd> --help` aunque el comando no los use.

`--json` sí cambia la salida (JSON en stdout, sin tablas) en: `doctor`, `lists`, `open`, `closed`, `cards *`, `working *`, `project *`, `objectives show`, `checkin`, `reminders`, `daily-work`, `users`, `theme list|set`, `update`, `board list`.

No aplica (siguen siendo UI o JSON-RPC): `tui`, `interactive`, `mcp`, `completions`. `login`, `board switch` y `board current` ignoran `--json` y escriben texto.

`--board` elige el board para listas/cards, `doctor` y `tui`. No cambia `working`, `reminders`, `checkin`, `users`, `daily-work`, `objectives`, `theme`, `update`, `login`, `mcp`, `completions` ni `project *` (estos últimos van por `project_id`).

```bash
hteam lists --json
hteam --board 500 lists
hteam --board 500 open
```

## Configuración

Archivo: `~/.config/hteam/config.toml`

```toml
[auth]
session_id = "…"
csrf_token = "…"
board_number = 483
user_id = 140

[boards]
483 = { name = "Care", last_used = "2026-06-09" }

[theme]
active = "default"

# Temas custom: ~/.config/hteam/themes/<name>.toml
# Objetivos semanales: sección [weekly_objectives]
```

No copies cookies reales al README ni a issues.

## Arquitectura

```
src/
├── main.rs
├── cli/             # clap, REPL, login
├── client/          # HTTP a hteam.mx
├── config/
├── mcp/             # servidor MCP (stdio)
├── models/
├── operations/      # lógica compartida (sin println/ratatui)
└── tui/             # kanban (ratatui)
```

## Dependencias principales

| Crate | Uso |
|---|---|
| `clap` | CLI |
| `clap_complete` | `hteam completions` |
| `reqwest` | HTTP + cookies |
| `tokio` | async |
| `serde` / `serde_json` | JSON |
| `toml` | config |
| `rmcp` | MCP |
| `tabled` | tablas |
| `rustyline` | REPL |
| `ratatui` / `crossterm` | TUI |

## Licencia

GNU General Public License v3.0 (GPL-3.0-only). Consulta [`LICENSE`](LICENSE) para el texto completo.
