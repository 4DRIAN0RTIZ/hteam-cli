# hteam

CLI en Rust para gestionar boards de Hteam, con soporte nativo de MCP (Model Context Protocol) para integración con asistentes de IA.

## Características

- **Autenticación** con cookies de sesión (`sessionid` + `csrftoken`)
- **Modo interactivo REPL** con historial persistente (`~/.hteam_history`)
- **Servidor MCP** sobre stdio — expone 20 herramientas para IA (Claude, etc.)
- **Multi-board** con persistencia de configuración
- **Salida dual**: tablas formateadas + JSON raw (`--json`) para scripting

## Instalación

```bash
git clone https://github.com/tu-usuario/hteam-cli.git
cd hteam-cli
cargo build --release
# Opcional: instalar globalmente
cargo install --path .
```

## Autenticación

```bash
hteam login
```

Solicita:
- **Session ID**: valor de la cookie `sessionid`
- **CSRF Token**: valor de la cookie `csrftoken`
- **Board number** (opcional): ID del board por defecto

## Comandos

### Board

```bash
hteam board list                          # Listar boards configurados
hteam board switch 483 --name "Care"      # Cambiar board activo
hteam board current                       # Mostrar board actual
```

### Listas y cards

```bash
hteam lists                               # Listar listas del board
hteam open                                # Cards abiertos (lista 1)
hteam closed                              # Cards cerrados (lista 2)
hteam cards list 1                        # Cards de una lista específica
hteam cards detail 12345                  # Detalle de un card
hteam cards labels 12345                  # Labels disponibles de un card
```

### Crear y modificar cards

```bash
# Crear card (en la lista por defecto del board)
hteam cards create --name "Fix login bug"
hteam cards create --name "Fix login bug" --list-id 3

# Mover card (--from es opcional, por defecto lista 1)
hteam cards move 12345 --to 2
hteam cards move 12345 --to 2 --from 1

# Actualizar descripción (rápido)
hteam cards update-desc 12345 --description "Nueva descripción"

# Actualizar campos completos (uno o más)
hteam cards update 12345 --name "Nuevo nombre"
hteam cards update 12345 --priority 1          # 1=alta 2=media 3=baja
hteam cards update 12345 --responsible usuario
hteam cards update 12345 --name "X" --description "Y" --priority 2

# Agregar comentario
hteam cards comment 12345 --text "Trabajando en esto"

# Crear recordatorio
hteam cards remind 12345
```

### Working On It

```bash
hteam working list                        # Ver en qué estás trabajando
hteam working start 12345                 # Empezar a trabajar (ID del card)
hteam working stop 8954                   # Dejar de trabajar (ID del *registro*, no del card)
```

> `working stop` recibe el ID del registro working-on-it (visible en `working list`), no el ID del card.

### Proyectos

```bash
hteam project milestones 99               # Progreso de milestones
hteam project tasks 99                    # Tasks del proyecto
```

### Usuarios y misc

```bash
hteam users "juan"                        # Buscar usuarios por nombre/username
hteam reminders                           # Ver recordatorios pendientes
hteam checkin                             # Registrar entrada del día
hteam daily-work                          # Historial de trabajo diario del equipo (hoy)
hteam daily-work --user juan --type time --range this_week
```

### Board visual (TUI)

```bash
hteam tui
```

Board estilo kanban en terminal (ratatui), con navegación por listas y cards del board activo.

### Autocompletado de shell

```bash
hteam completions zsh > _hteam            # bash, zsh, fish, elvish, powershell
```

### MCP Server

```bash
hteam mcp
```

Inicia el servidor MCP sobre **stdio**. Se integra con Claude Desktop, Claude Code u cualquier cliente MCP compatible. Las 22 herramientas expuestas son:

| Herramienta | Descripción |
|---|---|
| `lists` | Listas del board activo |
| `cards_list` | Cards de una lista |
| `cards_open` | Cards abiertos |
| `cards_closed` | Cards cerrados |
| `card_detail` | Detalle completo de un card |
| `card_labels` | Labels disponibles de un card |
| `card_create` | Crear card |
| `card_move` | Mover card entre listas |
| `card_update_desc` | Actualizar descripción |
| `card_update` | Actualizar nombre/descripción/prioridad/responsable |
| `card_comments` | Obtener comentarios de un card |
| `card_comment` | Agregar comentario |
| `card_remind` | Crear recordatorio |
| `working_list` | Working On It activo |
| `working_start` | Empezar a trabajar en un card |
| `working_stop` | Dejar de trabajar |
| `project_milestones` | Milestones de un proyecto |
| `project_tasks` | Tasks de un proyecto |
| `users_search` | Buscar usuarios |
| `reminders` | Recordatorios pendientes |
| `check_in` | Registrar entrada del día |
| `daily_work` | Historial de trabajo diario del equipo |

#### Configuración en Claude Desktop

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

#### Configuración en Claude Code

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

### Modo interactivo REPL

```bash
hteam interactive
```

Prompt: `hteam> `

| Comando | Descripción |
|---|---|
| `lists`, `ls` | Listar listas del board |
| `open`, `o` | Cards abiertos |
| `closed`, `c` | Cards cerrados |
| `cards <list_id>` | Cards de una lista |
| `create <nombre>`, `mk <nombre>` | Crear card |
| `move <id> --to <n>`, `mv` | Mover card |
| `detail <id>`, `show <id>` | Detalle de card |
| `comment <id> <texto>`, `co` | Agregar comentario |
| `working list`, `w list` | Working On It |
| `working start <card_id>` | Empezar a trabajar |
| `working stop <working_id>` | Dejar de trabajar |
| `board list` | Listar boards |
| `board switch <id>` | Cambiar board |
| `board current` | Board actual |
| `config show` | Mostrar configuración |
| `! <comando>` | Ejecutar comando shell |
| `clear`, `cls` | Limpiar pantalla |
| `help`, `h`, `?` | Ayuda |
| `exit`, `quit`, `q` | Salir |

### Salida JSON

Cualquier comando acepta `--json` para obtener la respuesta en JSON crudo:

```bash
hteam lists --json
hteam cards list 1 --json
hteam cards detail 12345 --json
```

### Flag global `--board`

Todos los comandos aceptan `--board <id>` para operar sobre un board distinto al configurado:

```bash
hteam --board 500 lists
hteam --board 500 open
```

## Configuración

Archivo: `~/.config/hteam/config.toml`

```toml
[auth]
session_id = "fuyjgvaarcoatxxk193d93dxai3p2ahd"
csrf_token  = "cgPg9XKgAJ8rOzRVDw03..."
board_number = 483
user_id = 140

[boards]
483 = { name = "Care",        last_used = "2026-06-09" }
500 = { name = "Development", last_used = "2026-06-01" }

[variables]
# Pares clave-valor personalizados (opcionales)
```

## Arquitectura

```
src/
├── main.rs          # Punto de entrada
├── cli/
│   ├── mod.rs       # Definición de comandos (clap)
│   ├── board.rs
│   ├── cards.rs
│   ├── working.rs
│   ├── project.rs
│   ├── login.rs
│   └── interactive.rs
├── client/
│   └── mod.rs       # Cliente HTTP (reqwest + cookies)
├── config/
│   └── mod.rs       # Configuración y persistencia TOML
├── mcp/
│   └── mod.rs       # Servidor MCP (rmcp, stdio)
├── models/
│   └── mod.rs       # Structs de datos
├── operations/      # Lógica de negocio compartida entre CLI, REPL, TUI y MCP
│   ├── mod.rs
│   ├── boards.rs
│   ├── cards.rs
│   ├── checkin.rs
│   ├── comments.rs
│   ├── daily_work.rs
│   ├── projects.rs
│   ├── reminders.rs
│   ├── session.rs
│   ├── users.rs
│   └── working.rs
└── tui/             # Board visual tipo kanban (ratatui)
    ├── mod.rs
    ├── app.rs
    ├── events.rs
    ├── ui.rs
    └── widgets/
        ├── mod.rs
        ├── popup.rs
        ├── scroll.rs
        └── text_input.rs
```

## Dependencias principales

| Crate | Uso |
|---|---|
| `clap` | CLI con derive macros |
| `reqwest` | Cliente HTTP con cookies |
| `tokio` | Runtime async |
| `serde` / `serde_json` | Serialización JSON |
| `toml` | Configuración TOML |
| `rmcp` | Servidor MCP (Model Context Protocol) |
| `schemars` | Esquemas JSON para herramientas MCP |
| `tabled` | Tablas formateadas en terminal |
| `rustyline` | REPL con historial |
| `inquire` | Prompts interactivos |
| `scraper` | Parseo HTML |
| `colored` | Colores en terminal |
| `chrono` | Fechas y horas |
| `anyhow` / `thiserror` | Manejo de errores |
| `ratatui` / `crossterm` | Board visual TUI (`hteam tui`) |
| `clap_complete` | Autocompletado de shell (`hteam completions`) |

## Licencia

GNU General Public License v3.0 (GPL-3.0-only). Consulta [`LICENSE`](LICENSE) para el texto completo.
