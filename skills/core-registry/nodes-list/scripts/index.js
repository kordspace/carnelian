/**
 * nodes-list skill wrapper
 * Category: automation
 * Ported from THUMMIM: nodes-tool.ts
 *
 * Sandbox globals available: WebSocket, crypto, process.env
 * Required env vars: GATEWAY_URL, GATEWAY_TOKEN
 */

// Shared WebSocket JSON-RPC helper
async function callGateway(method, params, opts = {}) {
  const gatewayUrl = process.env.GATEWAY_URL || "ws://127.0.0.1:18789";
  const gatewayToken = process.env.GATEWAY_TOKEN;
  const timeoutMs = opts.timeoutMs || 10000;
  
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(gatewayUrl);
    const requestId = crypto.randomUUID();
    let timeoutHandle;
    
    const cleanup = () => {
      if (timeoutHandle) clearTimeout(timeoutHandle);
      ws.close();
    };
    
    timeoutHandle = setTimeout(() => {
      cleanup();
      reject(new Error(`Gateway call timed out after ${timeoutMs}ms`));
    }, timeoutMs);
    
    ws.onopen = () => {
      // Send auth token if available
      if (gatewayToken) {
        ws.send(JSON.stringify({ type: "auth", token: gatewayToken }));
      }
      
      // Send JSON-RPC request
      ws.send(JSON.stringify({
        jsonrpc: "2.0",
        id: requestId,
        method,
        params,
      }));
    };
    
    ws.onmessage = (event) => {
      try {
        const parsed = JSON.parse(event.data);
        
        // Ignore non-matching responses
        if (parsed.id !== requestId) return;
        
        cleanup();
        
        if (parsed.error) {
          reject(new Error(parsed.error.message || JSON.stringify(parsed.error)));
        } else {
          resolve(parsed.result);
        }
      } catch (err) {
        cleanup();
        reject(err);
      }
    };
    
    ws.onerror = (err) => {
      cleanup();
      reject(new Error(`WebSocket error: ${err.message || "Unknown error"}`));
    };
  });
}

// Resolve node ID from identifier (nodeId, displayName, or remoteIp)
async function resolveNodeId(node) {
  if (!node) {
    throw new Error("Missing required field: node");
  }
  
  const nodeList = await callGateway("node.list", {});
  
  // Try exact nodeId match first
  let found = nodeList.nodes?.find(n => n.nodeId === node);
  if (found) return found.nodeId;
  
  // Try displayName match
  found = nodeList.nodes?.find(n => n.displayName === node);
  if (found) return found.nodeId;
  
  // Try remoteIp match
  found = nodeList.nodes?.find(n => n.remoteIp === node);
  if (found) return found.nodeId;
  
  throw new Error(`Node not found: ${node}`);
}

module.exports.run = async (input) => {
  // Validate input
  const action = input.action;
  
  if (!action) {
    throw new Error("Missing required field: action");
  }
  
  // Execute action
  switch (action) {
    case "status":
      return await callGateway("node.list", {});
    
    case "describe": {
      const nodeId = await resolveNodeId(input.node);
      return await callGateway("node.describe", { nodeId });
    }
    
    case "pending":
      return await callGateway("node.pair.list", {});
    
    case "approve":
      if (!input.requestId) {
        throw new Error("Missing required field: requestId");
      }
      return await callGateway("node.pair.approve", { requestId: input.requestId });
    
    case "reject":
      if (!input.requestId) {
        throw new Error("Missing required field: requestId");
      }
      return await callGateway("node.pair.reject", { requestId: input.requestId });
    
    case "notify": {
      if (!input.title || !input.body) {
        throw new Error("Missing required fields: title and body");
      }
      const nodeId = await resolveNodeId(input.node);
      return await callGateway("node.invoke", {
        nodeId,
        command: "system.notify",
        params: {
          title: input.title,
          body: input.body,
        },
        idempotencyKey: crypto.randomUUID(),
      });
    }
    
    case "camera_snap": {
      const nodeId = await resolveNodeId(input.node);
      const result = await callGateway("node.invoke", {
        nodeId,
        command: "camera.snap",
        params: input.params || {},
        idempotencyKey: crypto.randomUUID(),
      }, { timeoutMs: 30000 });
      
      return {
        base64: result.base64 || result.data,
        width: result.width,
        height: result.height,
        facing: result.facing,
      };
    }
    
    case "camera_list": {
      const nodeId = await resolveNodeId(input.node);
      return await callGateway("node.invoke", {
        nodeId,
        command: "camera.list",
        params: {},
        idempotencyKey: crypto.randomUUID(),
      });
    }
    
    case "camera_clip": {
      const nodeId = await resolveNodeId(input.node);
      const result = await callGateway("node.invoke", {
        nodeId,
        command: "camera.clip",
        params: input.params || {},
        idempotencyKey: crypto.randomUUID(),
      }, { timeoutMs: 60000 });
      
      return {
        base64: result.base64 || result.data,
        durationMs: result.durationMs,
        hasAudio: result.hasAudio,
      };
    }
    
    case "screen_record": {
      const nodeId = await resolveNodeId(input.node);
      const result = await callGateway("node.invoke", {
        nodeId,
        command: "screen.record",
        params: input.params || {},
        idempotencyKey: crypto.randomUUID(),
      }, { timeoutMs: 60000 });
      
      return {
        base64: result.base64 || result.data,
        durationMs: result.durationMs,
        fps: result.fps,
      };
    }
    
    case "location_get": {
      const nodeId = await resolveNodeId(input.node);
      return await callGateway("node.invoke", {
        nodeId,
        command: "location.get",
        params: {},
        idempotencyKey: crypto.randomUUID(),
      });
    }
    
    case "run": {
      if (!input.command || !Array.isArray(input.command)) {
        throw new Error("Missing required field: command (must be an array)");
      }
      const nodeId = await resolveNodeId(input.node);
      return await callGateway("node.invoke", {
        nodeId,
        command: "system.run",
        params: {
          command: input.command,
        },
        idempotencyKey: crypto.randomUUID(),
      }, { timeoutMs: 30000 });
    }
    
    default:
      throw new Error(`Unknown action: ${action}`);
  }
};
