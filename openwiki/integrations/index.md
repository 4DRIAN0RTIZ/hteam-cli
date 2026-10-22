# Files

- [Hteam API Integration](hteam-api.md) - HteamClient centralizes the authenticated Hteam API surface, including board discovery, card and project mutations, comment and working-state flows, and HTML-backed form scraping. It also records which endpoint family owns each field when data must be merged from multiple responses.
- [MCP Server Integration](mcp-server.md) - The Hteam MCP server exposes board, card, comment, working, project, and activity workflows over stdio using schema-driven tool parameters, thin delegation to shared operations, and uniform MCP error translation.
