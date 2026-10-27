---
type: Quickstart
title: Quickstart
description: Fast orientation for getting hteam running from a fresh checkout, authenticating safely, choosing a board, and routing to the CLI, TUI, REPL, or MCP docs.
tags: [quickstart, cli, tui, repl, mcp, authentication, board]
sources:
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
  - id: openwiki-source-730a6cb99277b4ce3292b050
    resource: repo://src/cli/interactive.rs
  - id: openwiki-source-2e5513f372a973635ee9a0c5
    resource: repo://src/cli/login.rs
  - id: openwiki-source-2d189e28b5edbe9715440baf
    resource: repo://src/cli/mod.rs
  - id: openwiki-source-3454eda491fb094684a56d2a
    resource: repo://src/client/mod.rs
  - id: openwiki-source-ec936990c4b0348483afdd8c
    resource: repo://src/config/mod.rs
  - id: openwiki-source-b55a21a31ede1b56cd31a6a6
    resource: repo://src/main.rs
  - id: openwiki-source-55d9f60b5eb1630419488cb7
    resource: repo://src/mcp/mod.rs
  - id: openwiki-source-a32e822ad3272b6a9b870734
    resource: repo://src/operations/cards.rs
  - id: openwiki-source-0fc29dd590a674bf167ededd
    resource: repo://src/operations/session.rs
  - id: openwiki-source-344d0ae26e653accaaef1142
    resource: repo://src/tui/mod.rs
generated: { by: "openwiki/0.7.2", at: "2026-10-27T23:32:56.589Z" }
---

# Quickstart

Use this page as the shortest path from a fresh checkout to a safe hteam session. It covers build and install, session cookies, login, board selection, and the first routing decision: which runtime surface you want next.

## Fast path

1. Build or install `hteam`.
2. Provide your Hteam session cookies.
3. Run `hteam login` to validate the session and resolve a default board.
4. Use the configured board, or pass `--board` when you start a surface that needs one.
5. Route to the CLI, TUI, interactive REPL, or MCP docs depending on your task.

```bash
cargo build --release
hteam login
hteam board current
hteam lists
hteam tui
hteam interactive
hteam mcp
```

## Install or build

From the repository root:

```bash
cargo build --release
```

If you want the command on your PATH, install it with Cargo:

```bash
cargo install --path .
```

## Configure credentials

`hteam` authenticates with the Hteam cookies `sessionid` and `csrftoken`.

You can pass them on the command line:

```bash
hteam login --session-id <sessionid> --csrf-token <csrftoken>
```

Or let the login command prompt for them interactively:

```bash
hteam login
```

The login flow probes authentication through the boards list endpoint with no hardcoded board. It keeps the on-disk config unchanged until that probe succeeds.

## Authenticate and select a board

After the cookie probe succeeds, login resolves the board to persist in this order:

1. An explicit `--board` if it is visible in the boards list.
2. The board already stored in config, if it is still visible.
3. The only visible board, when the account has exactly one.
4. Otherwise, no board is persisted and the command tells you to switch manually.

Typical output looks like this:

```text
Iniciando sesión en Hteam...

Verificando credenciales...
Autenticación exitosa!
Board activo: Care (483)
```

If there are several visible boards and no clear default, the command asks you to choose one later:

```text
Hay varios boards disponibles. Elige uno con `hteam board switch <id>`:

   483 - Care
   512 - Support
```

That behavior comes from `probe_auth`, session validation before persistence, and board resolution after auth. For the data model and selection rules behind that behavior, see [Session and Configuration](concepts/session-configuration.md).

## Pick the runtime surface

Once the board is configured, choose the surface that matches your task:

- **CLI** for one-off commands, tables, and `--json` output.
- **TUI** for a Kanban-style terminal board.
- **Interactive REPL** for a lightweight prompt, aliases, and history.
- **MCP** for stdio-based tool access from an external client.

### CLI

Use the command surface when you want a single result or scripted output. The dispatcher in `cli::run` routes conventional commands, plus the `Interactive`, `Tui`, and `Mcp` modes, from the same binary entrypoint.

```bash
hteam lists
hteam cards detail 12345
hteam cards move 12345 --to 2
```

Next: [Terminal Experiences](operations/terminal-experiences.md)

### TUI

Use the terminal UI when you want to browse and act on the active board interactively. It starts from the same configured session, and it needs a board number unless config already provides one.

```bash
hteam tui
```

Next: [Terminal Experiences](operations/terminal-experiences.md)

### Interactive REPL

Use the REPL for a prompt-driven session with shortcuts and persisted history. The REPL loads `~/.hteam_history` when available, saves it on exit, and delegates many actions to the same CLI handlers that power the command line.

```bash
hteam interactive
```

Next: [Terminal Experiences](operations/terminal-experiences.md)

### MCP

Use the MCP server when another tool should call hteam through stdio. The MCP surface is protocol-oriented and is the right next stop for tool mapping and transport details.

```bash
hteam mcp
```

Next: [MCP Server Integration](integrations/mcp-server.md)

## Where to go next in the wiki

- [System Overview](architecture/system-overview.md) for the runtime domains and ownership boundaries.
- [Session and Configuration](concepts/session-configuration.md) for persisted auth state, board selection, and local preferences.
- [Terminal Experiences](operations/terminal-experiences.md) for CLI, REPL, and TUI behavior.
- [MCP Server Integration](integrations/mcp-server.md) for the stdio server and tool surface.
- [Verification Strategy](testing/verification-strategy.md) for focused tests, mocked HTTP, and checkpoint-driven validation before you call a change done.
- [Card Mutations and Comments](workflows/card-mutations-and-comments.md) for the common edit flows across CLI, TUI, and MCP.
- [Login and Board Selection Workflow](workflows/login-and-board-selection.md) for the end-to-end auth and board-resolution path.
