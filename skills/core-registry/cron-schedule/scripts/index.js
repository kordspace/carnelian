/**
 * cron-schedule skill wrapper
 * Category: automation
 * Ported from THUMMIM: cron-tool.ts
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
    case "status":
      return await callGateway("cron.status", {});
    
    case "list":
      return await callGateway("cron.list", {});
    
    case "add":
      if (!input.job) {
        throw new Error("Missing required field: job");
      }
      return await callGateway("cron.add", { job: input.job });
    
    case "update":
      if (!input.jobId || !input.patch) {
        throw new Error("Missing required fields: jobId and patch");
      }
      return await callGateway("cron.update", {
        jobId: input.jobId,
        patch: input.patch,
      });
    
    case "remove":
      if (!input.jobId) {
        throw new Error("Missing required field: jobId");
      }
      return await callGateway("cron.remove", { jobId: input.jobId });
    
    case "run":
      if (!input.jobId) {
        throw new Error("Missing required field: jobId");
      }
      return await callGateway("cron.run", { jobId: input.jobId });
    
    case "runs":
      if (!input.jobId) {
        throw new Error("Missing required field: jobId");
      }
      return await callGateway("cron.runs", { jobId: input.jobId });
    
    case "wake":
      if (!input.text) {
        throw new Error("Missing required field: text");
      }
      return await callGateway("wake", {
        text: input.text,
        mode: input.mode,
      });
    
    default:
      throw new Error(`Unknown action: ${action}`);
  }
};
