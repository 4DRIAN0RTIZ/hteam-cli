# Files

- [Hteam API Integration](hteam-api.md) - HteamClient is the repository's HTTP boundary for Hteam. It centralizes auth probing, board and project routing, browser-form scraping, and the response-shape fallbacks the operations layer depends on.
- [MCP Server Integration](mcp-server.md) - The Hteam MCP server exposes board, card, comment, working, project, and activity workflows over stdio using schema-driven tool parameters, thin delegation to shared operations, and uniform MCP error translation.
