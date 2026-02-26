/**
 * gateway-query skill wrapper
 * Category: automation
 * Ported from THUMMIM: gateway-tool.ts
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

module.exports.run = async (input) => {
  // Validate input
  const action = input.action;
  
  if (!action) {
    throw new Error("Missing required field: action");
  }
  
  // Execute action
  switch (action) {
    case "config.get":
      return await callGateway("config.get", {});
    
    case "config.schema":
      return await callGateway("config.schema", {});
    
    case "config.apply": {
      if (!input.raw) {
        throw new Error("Missing required field: raw");
      }
      
      // Get baseHash if not provided
      let baseHash = input.baseHash;
      if (!baseHash) {
        const config = await callGateway("config.get", {});
        baseHash = config.hash;
      }
      
      return await callGateway("config.apply", {
        raw: input.raw,
        baseHash,
        sessionKey: input.sessionKey,
        note: input.note,
        restartDelayMs: input.restartDelayMs,
      });
    }
    
    case "config.patch": {
      if (!input.raw) {
        throw new Error("Missing required field: raw");
      }
      
      // Get baseHash if not provided
      let baseHash = input.baseHash;
      if (!baseHash) {
        const config = await callGateway("config.get", {});
        baseHash = config.hash;
      }
      
      return await callGateway("config.patch", {
        raw: input.raw,
        baseHash,
        sessionKey: input.sessionKey,
        note: input.note,
        restartDelayMs: input.restartDelayMs,
      });
    }
    
    case "update.run":
      return await callGateway("update.run", {
        sessionKey: input.sessionKey,
        note: input.note,
        restartDelayMs: input.restartDelayMs,
      });
    
    case "restart":
      // Fire-and-forget restart (WebSocket will close)
      return await callGateway("restart", {
        delayMs: input.delayMs,
        reason: input.reason,
      });
    
    default:
      throw new Error(`Unknown action: ${action}`);
  }
};
