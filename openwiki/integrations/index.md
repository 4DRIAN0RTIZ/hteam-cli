# Files

- [Hteam HTTP integration contract](hteam-api.md) - HteamClient is the single HTTP boundary to hteam.mx, centralizing authenticated requests, Hteam-specific payloads and parsing, identifier discovery, and transport failures for CLI, TUI, and MCP workflows.
- [MCP stdio server and tool surface](mcp-server.md) - The hteam MCP server exposes the shared Hteam board operations over stdio, with schema-derived tool parameters, JSON text results, and uniform MCP internal-error translation.
