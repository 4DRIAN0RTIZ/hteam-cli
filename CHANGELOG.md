# Changelog

All notable changes to hteam-cli will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `hteam daily-work` CLI command — historial de trabajo diario del equipo (`GET /history/daily-work`, filtros por usuario, tipo de actividad y rango de tiempo)
- `daily_work` MCP tool, espejo del comando CLI
- `DailyWorkEntry` model
- Timeout global de 30s en el cliente HTTP (`reqwest`)

### Fixed

- `get_board_id()` ya no asume que la lista 1 ("Open") tiene cards: si está vacía, recorre el resto de listas hasta encontrar una con `card_count > 0`
- `create_card()` ya no se auto-deadlockea — soltaba el lock de `tokio::sync::Mutex<Config>` antes de llamar a `get_board_id()`, que también lo necesita

## [0.1.0] - 2026-06-16

### Added

- Servidor MCP y CLI base para hteam (autenticación, tarjetas, listas, comentarios, recordatorios, check-in, working on/off)
- Soporte para obtener comentarios de un card vía `django-comments-xtd`
