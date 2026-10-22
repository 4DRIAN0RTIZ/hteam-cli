---
type: Quickstart
title: Quickstart
description: Concise onboarding for installing hteam, configuring credentials, authenticating, choosing a board, and entering the CLI, TUI, interactive REPL, or MCP server.
tags: [quickstart, cli, tui, mcp, authentication]
verified:
  - by: openwiki/0.7.2
    at: 2026-08-29T04:34:19.305Z
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
generated: { by: "openwiki/0.7.2", at: "2026-08-29T04:34:19.305Z" }
---

# Quickstart

Use this page as the shortest path from a fresh checkout to a working session. It gets you through build, credentials, login, board selection, and then points you at the runtime surface you want to use next.

## Fast path

1. Build or install the binary.
2. Provide your Hteam session cookies.
3. Run `hteam login` to validate the session and pick a board.
4. Use the board from config or pass `--board` when you start the CLI, TUI, interactive REPL, or MCP server.

Plain-text example:

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
- **Interactive REPL** for a lightweight prompt and command aliases.
- **MCP** for stdio-based tool access from an external client.

### CLI

Start with the command surface when you want a single result or scripted output.

```bash
hteam lists
hteam cards detail 12345
hteam cards move 12345 --to 2
```

Next: [Terminal Experiences](operations/terminal-experiences.md)

### TUI

Use the terminal UI when you want to browse and act on the active board interactively.

```bash
hteam tui
```

Next: [Terminal Experiences](operations/terminal-experiences.md)

### Interactive REPL

Use the REPL for a prompt-driven session with shortcuts and history.

```bash
hteam interactive
```

Next: [Terminal Experiences](operations/terminal-experiences.md)

### MCP

Use the MCP server when another tool should call hteam through stdio.

```bash
hteam mcp
```

Next: [MCP Server Integration](integrations/mcp-server.md)

## Where to go next in the wiki

- [System Overview](architecture/system-overview.md) for the runtime domains and ownership boundaries.
- [Session and Configuration](concepts/session-configuration.md) for persisted auth state, board selection, and local preferences.
- [Terminal Experiences](operations/terminal-experiences.md) for CLI, REPL, and TUI behavior.
- [MCP Server Integration](integrations/mcp-server.md) for the stdio server and tool surface.
- [Verification Strategy](testing/verification-strategy.md) for the tests and checks that matter before you claim a change is done.
