---
type: workflow
title: Login and Board Selection Workflow
description: End-to-end login flow from cookie prompts through auth probing, board resolution, and the exact config state that is persisted after success.
tags: [login, auth, board-selection, workflows]
verified:
  - by: openwiki/0.7.2
    at: 2026-10-27T23:40:03.433Z
sources:
  - id: openwiki-source-2e5513f372a973635ee9a0c5
    resource: repo://src/cli/login.rs
  - id: openwiki-source-ec936990c4b0348483afdd8c
    resource: repo://src/config/mod.rs
  - id: openwiki-source-d7c0e68afeedea2d27dd68f8
    resource: repo://src/operations/boards.rs
  - id: openwiki-source-0fc29dd590a674bf167ededd
    resource: repo://src/operations/session.rs
generated: { by: "openwiki/0.7.2", at: "2026-10-27T23:40:03.433Z" }
---

# Login and Board Selection Workflow

`hteam login` is a guarded persistence flow: it first collects cookies, then probes the live boards endpoint, and only after a successful probe does it write session data to local config. Board selection is resolved from the server's visible boards, with explicit override first, then a still-visible configured board, then single-board auto-pick, and finally a prompt to switch manually when the choice is ambiguous.

## Control flow

<!-- openwiki: mermaid parse failed and this diagram was converted to a text fence so it does not break rendering. Fix the diagram source and restore the mermaid fence. Parser error: Heuristic: a semicolon inside a label breaks rendering; rephrase the label. -->
```text
flowchart TD
    A[Start login] --> B{sessionid provided?}
    B -- no --> C[Prompt for sessionid]
    B -- yes --> D[Use sessionid flag]
    C --> E{csrftoken provided?}
    D --> E
    E -- no --> F[Prompt for csrftoken]
    E -- yes --> G[Load config in memory]
    F --> G
    G --> H[Clone config and attach probe cookies]
    H --> I[Call check_auth via HteamClient]
    I --> J{AuthCheck}
    J -- Unauthorized --> K[Error out; keep disk config unchanged]
    J -- Err --> L[Error out; keep disk config unchanged]
    J -- Ok --> M[Resolve board from visible boards]
    M --> N{Resolution result}
    N -- Selected --> O[Persist cookies and selected board]
    N -- NeedsSwitch --> P[Persist cookies only]
    N -- NoneAvailable --> P
    N -- NotFound --> P
    O --> Q[Print success and active board]
    P --> R[Print success plus board-specific follow-up]
```

This diagram shows the real fallback chain: cookies are prompted first, auth is probed before any write, and board persistence only happens when a board can be named unambiguously.

## Entry point and auth probe

The CLI entrypoint is [`src/cli/login.rs`](../../src/cli/login.rs). It accepts `--session-id`, `--csrf-token`, and optional `--board`; if either cookie is absent, the command prompts interactively with `inquire::Text`.

Before it mutates anything on disk, the command loads `Config` and clones it into a temporary probe config with the supplied cookies attached. That probe config is passed to `HteamClient::new`, and `operations::session::check_auth` performs the actual validation against the boards list endpoint. The probe is therefore based on the visible `Execution` boards, not on a hardcoded board id.

Failure handling is intentionally strict:

- `AuthCheck::Unauthorized` becomes a user-facing invalid-session error.
- Transport or network failures are surfaced as a distinct auth-check failure.
- In both cases, the existing on-disk config is left untouched.

## Board resolution chain

Once the auth probe returns `Ok`, `resolve_login_board` decides what, if anything, should become the active board.

1. An explicit `--board` wins when that id appears in the visible boards list.
2. Otherwise, a configured board is reused only if it is still visible.
3. If exactly one board is visible, login auto-selects it.
4. If no boards are visible, the session is still valid but no board can be selected.
5. If more than one board is visible and nothing is selected yet, login leaves the user to choose later.
6. If `--board` names an id that is not visible, the session is still valid, but the requested board is reported as missing.

That logic means login can succeed without producing a default board. The command may authenticate successfully and still ask the user to run `hteam board switch <id>` afterward.

## Persistence rules

Persistence happens through `operations::session::persist_login` after the auth probe succeeds.

What gets written on success:

- `auth.session_id`
- `auth.csrf_token`
- the config file itself
- the active board number when resolution returns `Selected`
- the last-ticket file for the selected board
- the board cache entry, including the board name and a fresh `last_used` timestamp

What never gets written on failure:

- new session cookies
- a new board selection
- the last-ticket file
- the board cache entry
- any config file changes at all

The important boundary is that invalid cookies never replace the previous stored session. The code keeps the original disk config in memory until the probe succeeds, and only then calls `persist_login`.

When resolution returns `NeedsSwitch`, `NoneAvailable`, or `NotFound`, the login command still persists the cookies but intentionally skips board persistence. That preserves a valid session while leaving board choice for a later manual step.

## User-visible outcomes

After a successful auth probe, the command prints `Autenticación exitosa!` and then reacts to the resolution result:

- **Selected board**: prints the board name and id.
- **NeedsSwitch**: prints the visible boards and tells the user to run `hteam board switch <id>`.
- **NoneAvailable**: prints that the session is valid but no `Execution` boards are visible.
- **NotFound**: saves the session, then fails with a message that the requested board is not in the visible list.

## Relationship to board switching

This workflow shares its board-persistence behavior with [`src/operations/boards.rs`](../../src/operations/boards.rs): both login and manual board switching update `config.toml`, save the last selected board, and refresh the in-memory board cache entry when a board is chosen. Login is simply the place where session validity is established before the default board preference is resolved.

## Tests that matter

The session tests verify the pieces that define this workflow:

- `check_auth` returns visible boards on a 200 response.
- `check_auth` maps 403 responses to `Unauthorized`.
- `resolve_login_board` prefers explicit board ids, then a visible configured board, then a single visible board.
- `resolve_login_board` falls back to `NeedsSwitch`, `NoneAvailable`, or `NotFound` in the remaining cases.

These tests are the main guardrails for the login workflow's observable behavior and its no-write-on-failure invariant.
