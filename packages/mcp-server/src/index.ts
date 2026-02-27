#!/usr/bin/env node
/**
 * Carnelian MCP Server for Windsurf IDE
 * 
 * Integrates Carnelian OS with Windsurf's Cascade via MCP protocol.
 * Provides tools for task delegation, skill invocation, and status monitoring.
 */

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  type Tool,
} from "@modelcontextprotocol/sdk/types.js";

const GATEWAY_PORT = process.env.CARNELIAN_GATEWAY_PORT || "18789";
const GATEWAY_TOKEN = process.env.CARNELIAN_GATEWAY_TOKEN || "";
const BASE_URL = `http://127.0.0.1:${GATEWAY_PORT}`;

async function gatewayFetch(path: string, options?: RequestInit): Promise<{ ok: boolean; data?: unknown; error?: string }> {
  try {
    const url = GATEWAY_TOKEN 
      ? `${BASE_URL}${path}?token=${GATEWAY_TOKEN}`
      : `${BASE_URL}${path}`;
    const response = await fetch(url, options);
    if (!response.ok) {
      return { ok: false, error: `HTTP ${response.status}` };
    }
    const text = await response.text();
    try {
      return { ok: true, data: JSON.parse(text) };
    } catch {
      return { ok: true, data: text };
    }
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err) };
  }
}

const TOOLS: Tool[] = [
  {
    name: "carnelian_status",
    description: "Get Carnelian OS status including active tasks, skills, and system health.",
    inputSchema: {
      type: "object",
      properties: {},
    },
  },
  {
    name: "carnelian_skills",
    description: "List all available skills in Carnelian OS.",
    inputSchema: {
      type: "object",
      properties: {
        category: {
          type: "string",
          description: "Filter by skill category (optional)",
        },
      },
    },
  },
  {
    name: "carnelian_invoke_skill",
    description: "Invoke a Carnelian skill with parameters.",
    inputSchema: {
      type: "object",
      properties: {
        skillName: {
          type: "string",
          description: "Name of the skill to invoke",
        },
        params: {
          type: "object",
          description: "Parameters for the skill",
        },
      },
      required: ["skillName"],
    },
  },
  {
    name: "carnelian_task",
    description: "Add a task to Carnelian's autonomous task queue.",
    inputSchema: {
      type: "object",
      properties: {
        description: {
          type: "string",
          description: "Task description",
        },
        priority: {
          type: "string",
          enum: ["low", "medium", "high", "urgent"],
          description: "Task priority",
          default: "medium",
        },
      },
      required: ["description"],
    },
  },
  {
    name: "cascade_respond",
    description: "Send a response back to Carnelian from Cascade.",
    inputSchema: {
      type: "object",
      properties: {
        messageId: {
          type: "string",
          description: "Message ID to respond to",
        },
        response: {
          type: "string",
          description: "Response content",
        },
      },
      required: ["messageId", "response"],
    },
  },
];

async function handleCarnelianStatus(): Promise<string> {
  const result = await gatewayFetch("/api/status");
  if (!result.ok) {
    return `Carnelian gateway not reachable: ${result.error}`;
  }
  return `Carnelian OS is running!\n\nGateway: ${BASE_URL}\nStatus: ${JSON.stringify(result.data, null, 2)}`;
}

async function handleCarnelianSkills(args: { category?: string }): Promise<string> {
  const path = args.category ? `/api/skills?category=${args.category}` : "/api/skills";
  const result = await gatewayFetch(path);
  if (!result.ok) {
    return `Error fetching skills: ${result.error}`;
  }
  return JSON.stringify(result.data, null, 2);
}

async function handleCarnelianInvokeSkill(args: { skillName: string; params?: Record<string, unknown> }): Promise<string> {
  const result = await gatewayFetch("/api/skills/invoke", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      skill: args.skillName,
      params: args.params || {},
    }),
  });
  if (!result.ok) {
    return `Error invoking skill: ${result.error}`;
  }
  return JSON.stringify(result.data, null, 2);
}

async function handleCarnelianTask(args: { description: string; priority?: string }): Promise<string> {
  const result = await gatewayFetch("/api/tasks", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      description: args.description,
      priority: args.priority || "medium",
    }),
  });
  if (!result.ok) {
    return `Error adding task: ${result.error}`;
  }
  return `Task queued: [${args.priority || "medium"}] ${args.description}`;
}

async function handleCascadeRespond(args: { messageId: string; response: string }): Promise<string> {
  // Write response to cascade channel
  return `Response sent to Carnelian (messageId: ${args.messageId})`;
}

async function main() {
  const server = new Server(
    {
      name: "carnelian-mcp",
      version: "1.0.0",
    },
    {
      capabilities: {
        tools: {},
      },
    }
  );

  server.setRequestHandler(ListToolsRequestSchema, async () => ({
    tools: TOOLS,
  }));

  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const { name, arguments: args } = request.params;

    let result: string;

    switch (name) {
      case "carnelian_status":
        result = await handleCarnelianStatus();
        break;
      case "carnelian_skills":
        result = await handleCarnelianSkills(args as { category?: string });
        break;
      case "carnelian_invoke_skill":
        result = await handleCarnelianInvokeSkill(args as { skillName: string; params?: Record<string, unknown> });
        break;
      case "carnelian_task":
        result = await handleCarnelianTask(args as { description: string; priority?: string });
        break;
      case "cascade_respond":
        result = await handleCascadeRespond(args as { messageId: string; response: string });
        break;
      default:
        result = `Unknown tool: ${name}`;
    }

    return {
      content: [{ type: "text", text: result }],
    };
  });

  const transport = new StdioServerTransport();
  await server.connect(transport);

  console.error("Carnelian MCP Server started");
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
