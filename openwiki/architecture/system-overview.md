---
type: Reference
title: System overview
description: Architecture overview for the Hteam Rust binary. Explains how CLI, REPL, TUI, and MCP entrypoints share operations, and where configuration, session, and transport state are owned.
tags: [architecture, delivery, client, operations, config, runtime]
verified:
  - by: openwiki/0.7.2
    at: 2026-08-29T04:34:19.305Z
sources:
  - id: openwiki-source-730a6cb99277b4ce3292b050
    resource: repo://src/cli/interactive.rs
  - id: openwiki-source-2e5513f372a973635ee9a0c5
    resource: repo://src/cli/login.rs
  - id: openwiki-source-2d189e28b5edbe9715440baf
    resource: repo://src/cli/mod.rs
  - id: openwiki-source-3454eda491fb094684a56d2a
    resource: repo://src/client/mod.rs
  - id: openwiki-source-b55a21a31ede1b56cd31a6a6
    resource: repo://src/main.rs
  - id: openwiki-source-55d9f60b5eb1630419488cb7
    resource: repo://src/mcp/mod.rs
  - id: openwiki-source-d7c0e68afeedea2d27dd68f8
    resource: repo://src/operations/boards.rs
  - id: openwiki-source-a32e822ad3272b6a9b870734
    resource: repo://src/operations/cards.rs
  - id: openwiki-source-0fc29dd590a674bf167ededd
    resource: repo://src/operations/session.rs
generated: { by: "openwiki/0.7.2", at: "2026-08-29T04:34:19.305Z" }
---

# System overview

The `hteam` binary has one application core shared by several terminal-facing delivery mechanisms. `main` starts the Tokio runtime and immediately calls `cli::run()`. From there, Clap dispatches conventional commands plus the long-lived `Interactive`, `Tui`, and `Mcp` modes. These are alternative presentation adapters, not separate implementations of Hteam behavior.

## Runtime domains

| Domain | Owns | Must not own |
| --- | --- | --- |
| CLI, REPL, TUI, MCP | Input parsing, protocol/UI event handling, rendering, and translating results into native output | Business decisions or direct HTTP calls to Hteam |
| `operations` | Shared use cases and cross-surface rules, expressed in domain values | `println!`, Ratatui state/widgets, MCP result formatting, or transport implementation |
| `client::HteamClient` | Hteam request construction, authentication headers/cookies, response status handling, endpoint quirks, and in-memory client cache | Terminal presentation or feature-specific UI policy |
| `config::Config` | Local user state on disk | Remote API transport |
| `models` | Serde representations and normalization of Hteam payload shapes | Network requests and workflow/presentation policy |

```mermaid
flowchart TD
    User["User or MCP host"] --> CLI["CLI command parser"]
    User --> REPL["Interactive REPL"]
    User --> TUI["Ratatui TUI"]
    User --> MCP["MCP stdio server"]
    CLI --> OPS["operations shared use cases"]
    REPL --> CLI
    TUI --> OPS
    MCP --> OPS
    OPS --> CLIENT["HteamClient transport"]
    OPS --> CONFIG["Config local persistence"]
    CLIENT --> CONFIG
    CLIENT --> MODELS["models payload types"]
    CLIENT --> API["hteam.mx API and site endpoints"]
```

This diagram shows the intended request path: adapters present inputs and results, shared operations coordinate use cases, and only `HteamClient` crosses the remote boundary.

## Entrypoints and session lifecycle

`#[tokio::main]` calls `cli::run()`. The CLI command tree includes ordinary command-line actions plus `Interactive`, `Tui`, and `Mcp` delivery modes. The MCP path opens a session once and then serves tools over stdio; the TUI opens a session, resolves a board, loads state, enters the alternate terminal screen, and runs its event loop.

`operations::Session::open()` is the common authenticated composition point. It loads `Config`, clones it into `HteamClient::with_auth`, and returns both values. Authentication requires both `session_id` and `csrf_token`; the client rejects an unauthenticated configuration before a request is made. Login is an adapter-level setup flow: it collects or accepts cookie values, optionally records the selected board, tests them through `HteamClient`, and saves only after successful verification.

**Change rule:** use `Session::open()` for a normal authenticated adapter entrypoint. Do not reimplement config loading, authentication checks, or client construction independently unless the use case has a concrete lifecycle reason.

## Shared operations contract

`operations` is the seam for behavior that multiple delivery surfaces should share. Its modules cover boards, cards, check-in, comments, daily work, projects, reminders, users, working-on-it, and update-related helpers. Functions accept `&HteamClient`, plus `&mut Config` where a preference must be persisted, and return plain domain data or `Result<()>`. The MCP server demonstrates the intended boundary: tool handlers deserialize parameters, call `operations`, and serialize the result with MCP-specific `CallToolResult` formatting.

Operations may establish meaningful defaults and multi-step workflows. For example, `operations::cards::move_card` defaults an omitted source list to Open (`1`), and `update_card_fields` / `update_card_full` GET the HTML task edit form, scrape a baseline, apply only the caller's patch, and POST the full form so unspecified fields are preserved. Task JSON alone is not enough for partial edits such as priority, dependence id, estimate, and `YYYY-MM-DD` dates. This policy belongs in operations/client because every surface needs the same outcome. Tests verify the default move payload and the form-preserving update sequence.

Board choice is another shared invariant. `resolve_board_number` uses this precedence:

1. an explicit adapter override;
2. `Config.auth.board_number`;
3. the last board recorded in `~/.last_ticket`.

The TUI uses that resolver before it begins. Board switching updates the configured board, writes the last-ticket file, upserts board display metadata, and persists `config.toml`. Do not create feature-local board fallback chains.

## Transport and remote-state boundary

`HteamClient` owns the concrete `reqwest` client, base URLs, a mutex-protected in-memory `Config`, and all Hteam endpoint interaction. It builds a consistent request identity: referer, manually constructed `sessionid`/`csrftoken` cookie header, `X-CSRFToken`, optional bearer token, AJAX header, and accept header. Its 30-second client timeout and contextual errors make transport failures observable at the adapter boundary.

The client also contains remote protocol accommodations that must not leak upward:

- It selects the configured or last-used board when an endpoint receives no board argument.
- It caches `board_id` and invalidates it when the board number changes, preventing create/move actions from using an ID from the previous board.
- It discovers and persists `user_id` when required, first from working-on-it data and then from the latest work-shift record.
- It accepts working-on-it and reminder responses that may be empty, `null`, or multiple supported JSON shapes.
- Comment posting fetches the task page first to extract the form tokens required by the site endpoint, then sends the encoded form.

These are transport and protocol responsibilities even when they involve several requests.

**Non-negotiable boundary:** never add a `reqwest` call, Hteam URL, request header or cookie construction, or API-response workaround to `cli/`, `tui/`, `mcp/`, or the REPL. Add or extend an `HteamClient` method, expose the reusable behavior from `operations` where appropriate, then let each adapter present the returned result. Likewise, do not put a business decision in a delivery adapter merely because it is convenient for one interface.

## Local persistence and configuration

`Config` is the local state owner. It reads and writes TOML at the platform configuration directory under `hteam/config.toml`; a missing file produces defaults, while read, parse, directory-creation, serialization, and write failures are propagated with context. Persisted state includes authentication material, default board and cached IDs, named-board metadata, variables, TUI preferences, working-hour settings, weekly objectives, theme selection, and update-check state. The separate `~/.last_ticket` file stores the fallback board used by session resolution.

A client receives a cloned configuration and protects its own in-memory copy with `Arc<Mutex<Config>>`. That means a caller that changes a separate `Session.config` must deliberately persist it through the operation that owns that transition. Conversely, client-side caches such as discovered `user_id` can save themselves. Treat this as an ownership and lifecycle distinction rather than assuming all copies update automatically.

## Models as the payload boundary

`models` prevents external JSON details from becoming application-wide assumptions. It maps API field names such as card `title`, `subtitle`, `labels_list`, and list `total_board_cards` to stable Rust fields. It also deliberately normalizes irregular payloads: card-detail labels tolerate an array of label objects as well as scalar or string forms by yielding an empty label set for unsupported forms; working-on-it responses become `WorkingOnStatus`; project milestones deserialize from tuple-shaped API entries and normalize their progress representation for display.

Keep this boundary honest: add serde aliases, custom deserialization, or conversion types here, or immediately alongside transport parsing, when Hteam payload variance is discovered. Do not make CLI, TUI, or MCP consumers parse `serde_json::Value` or compensate independently.

## Adapter-specific state and failures

Adapters are free to own ephemeral presentation state. The TUI owns selection, dialog state, visible-list filtering, redraw timing, and terminal cleanup. It performs an optimistic card move for responsiveness, but reverts its local card placement when the shared operation fails; the remote mutation still flows through `operations` and `HteamClient`. Its hidden-list preference is persisted through `Config` after the UI decision is made.

MCP owns schema and parameter decoding and converts `anyhow` failures to MCP internal errors; it serializes successful domain values as JSON text content. The CLI owns Clap parsing and chooses JSON, tables, or messages. The REPL owns readline history in `~/.hteam_history` and command shortcuts, then delegates to CLI handlers. No adapter should let rendering, a partial refresh failure, or an interface protocol change redefine core business rules.

## Safe extension checklist

1. Define the operation in `operations` when the behavior, default, validation, or sequence is reusable; return models or plain values.
2. Put every new Hteam request, auth or header requirement, response status check, and response-shape accommodation in `HteamClient`.
3. Extend `models` for external payload mapping and normalization.
4. Put durable user preference or credential changes in `Config` and make persistence explicit.
5. Add thin adapter glue for CLI output, TUI interaction, REPL routing, or MCP schemas, not another implementation of the use case.
6. Add focused tests at the ownership seam: operations tests use `HteamClient::new_for_test` with Mockito to assert request paths and payloads, and pure configuration or resolver rules use ordinary unit tests.

For MCP-specific tool availability and stdio operation, see [MCP server](../integrations/mcp-server.md). For board and card terminology, see [cards and board state](../concepts/cards-and-board-state.md).
