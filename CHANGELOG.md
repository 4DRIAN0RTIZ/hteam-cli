# Changelog

All notable changes to hteam-cli will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Features
- *(cards)* Support project board lists and task detail
- *(projects)* Create and edit milestones
- *(projects)* Create tasks in milestones
- *(projects)* Create tasks in milestones
- *(projects)* Edit project tasks
- *(projects)* Add project task dependency tree
- *(projects)* Add project status summary
- *(projects)* Manage project members
- *(projects)* Delete empty milestones
- *(tui)* Create and edit project milestones

## [0.12.0] - 2026-10-23

### Documentation
- *(readme)* Count 31 MCP tools after dashboard
- *(readme)* Document clap commands and snapshot completions
- *(cli)* Merge dashboard into completions and README


### Features
- *(cli)* Add personal dashboard command
- *(comments)* Support replying to card comments
- *(reminders)* Support reminder cancellation
- *(cards)* Support card label mutations

## [0.11.0] - 2026-10-22

### Bug Fixes
- *(login)* Validate session without hardcoded board
- *(config)* Write credentials atomically with private permissions
- *(config)* Tighten existing hteam dir and leftover temp files
- *(config)* Use unique temp files when saving credentials
- *(config)* Remove leftover temps from dead processes


### Documentation
- *(openwiki)* Refresh login probe and CLI session docs


### Features
- *(cli)* Add hteam doctor for config and session checks

## [0.10.0] - 2026-10-14

### Bug Fixes
- *(client)* Fetch all cards when list exceeds default page size
- *(i18n)* Use neutral Spanish in update prompts
- *(tui)* Load times and labels from board-card into detail
- *(tui)* Scroll list columns so the selected card stays visible
- *(tui)* Show depends-on display string from task detail API
- *(client)* Truncate start_date on task edit form
- *(client)* Preserve form fields on partial task edit
- *(tui)* Show and refresh priority from HTML edit form


### Documentation
- *(client)* Translate cards pagination comment to English
- *(openwiki)* Document HTML task edit form contract


### Features
- *(tui)* Keep board interactive during network waits
- *(tui)* Debounce mention user search while typing
- *(tui)* Load board lists in parallel without dropping failed data
- *(tui)* Show changelog and update-available popups on startup
- *(operations)* Add shared card search and filter layer
- *(cards)* Expose shared search on CLI and MCP
- *(tui)* Filter board cards with / search
- *(tui)* Task detail popup with field edit
- *(tui)* Show and edit task dependence in detail
- *(tui)* Dependence task picker and correct empty clear
- *(cards)* Add close and reopen via task API families
- *(cards)* Edit start and due dates
- *(cards)* Edit start and due dates
- *(cards)* Record estimates and spent time
- *(cards)* Record estimates and spent time

## [0.9.1] - 2026-10-01

### Bug Fixes
- *(tui)* Size board cards by wrapped title lines


### Documentation
- *(openwiki)* Refresh architecture and quickstart pages

## [0.9.0] - 2026-09-11

### Features
- *(cards)* Add copy-link command for cards and projects
- *(theme)* Add custom theme files with full background/foreground control
- *(project)* Add project list with status filter, contextual popup help

## [0.8.0] - 2026-09-04

### Bug Fixes
- *(client)* Parse Sept. and noon/midnight timestamps in daily-work


### Features
- *(theme)* Add configurable TUI color presets
- *(update)* Add self-update command and version notices
- *(cards)* Add follow-up tracking with complete/cancel actions

## [0.7.0] - 2026-08-31

### Bug Fixes
- *(tui)* Omit empty task placeholder in weekly objectives


### Features
- *(objectives)* Add weekly objectives sharepad command
- *(tui)* Add weekly objectives popup

## [0.6.2] - 2026-08-29

### Bug Fixes
- *(client)* Replace useless format! calls with to_string

## [0.6.1] - 2026-08-29

### Documentation
- *(openwiki)* Add generated repository knowledge base

## [0.6.0] - 2026-08-17

### Bug Fixes
- *(release)* Pass --tag when generating docs/changelog.json
- *(client)* Resolve board id via get_board_id in get_card_labels


### Features
- *(comments)* Add follow-up flag and custom date to comment posting

## [0.5.2] - 2026-08-17

### Bug Fixes
- *(client)* Sync board_number when switching boards in the TUI

## [0.5.1] - 2026-08-15

### Documentation
- *(readme)* Sync README with current MCP tools, CLI commands and license


### Features
- *(tui)* Add live board selector
- *(tui)* Add scroll support to popups and project detail focus
- *(tui)* Show version in status bar


### Refactoring
- *(core)* Extract shared operations and tui widgets modules

## [0.5.0] - 2026-08-14

### Features
- *(workshift)* Show current shift status in the TUI header
- *(tui)* Display remaining working hours in title bar

## [0.4.0] - 2026-08-13

### Features
- Add inline description editing and automate releases

## [0.3.0] - 2026-08-12

### Features
- *(tui)* Add interactive board terminal interface

## [0.2.0] - 2026-08-07

### Features
- *(cli)* Add shell completion generation

## [0.1.0] - 2026-08-07

### Bug Fixes
- *(ci)* Create docs directory before generating changelog.json


### Features
- MCP server y CLI para hteam
- Obtener comentarios de un card via django-comments-xtd
- *(daily-work)* Add daily work history to CLI and MCP

---

## Version Numbering

This project follows [Semantic Versioning](https://semver.org/):

- **MAJOR**: Incompatible API changes or significant breaking changes
- **MINOR**: New functionality in a backwards compatible manner
- **PATCH**: Backwards compatible bug fixes
