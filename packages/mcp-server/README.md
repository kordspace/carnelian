# Carnelian MCP Server for Windsurf

Integrate **Carnelian OS** with Windsurf IDE's Cascade via the Model Context Protocol (MCP).

## Features

This MCP server provides the following tools to Cascade:

| Tool | Description |
|------|-------------|
| `carnelian_status` | Get Carnelian's current status and system health |
| `carnelian_skills` | List available skills Carnelian can use |
| `carnelian_invoke_skill` | Invoke a Carnelian skill with parameters |
| `carnelian_task` | Add a task to Carnelian's autonomous task queue |
| `cascade_respond` | Send a response back to Carnelian from Cascade |

## Installation

### From source

```bash
cd packages/mcp-server
npm install
npm run build
```

## Configuration

Add to your Windsurf MCP config at `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "carnelian": {
      "command": "node",
      "args": ["C:/path/to/CARNELIAN/packages/mcp-server/dist/index.js"],
      "env": {
        "CARNELIAN_GATEWAY_PORT": "18789",
        "CARNELIAN_GATEWAY_TOKEN": "YOUR_GATEWAY_TOKEN"
      }
    }
  }
}
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CARNELIAN_GATEWAY_PORT` | HTTP port of Carnelian's gateway | `18789` |
| `CARNELIAN_GATEWAY_TOKEN` | Authentication token for the gateway | (none) |

## Usage Examples

Once configured, you can use these tools in Windsurf Cascade:

### Check Carnelian Status
```
Use carnelian_status to see if Carnelian is running
```

### List Skills
```
Use carnelian_skills to see available skills
```

### Invoke a Skill
```
Use carnelian_invoke_skill with skillName "github-create-pr" and params {"title": "Fix bug", "body": "Description"}
```

### Add a Task
```
Use carnelian_task to add: "Review the test coverage" with priority high
```

## Troubleshooting

### Connection Failed
- Ensure Carnelian is running: check `http://localhost:18789/health`
- Verify the gateway port and token are correct

### Authentication Error
- Make sure `CARNELIAN_GATEWAY_TOKEN` matches your gateway config

### Tools Not Appearing
- Restart Windsurf after updating `mcp_config.json`
- Check Windsurf's MCP settings panel for errors

## License

MIT
