/**
 * session-spawn skill wrapper
 * Category: automation
 * Ported from THUMMIM: session-spawn-tool.ts
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
  const task = input.task;
  
  if (!task) {
    throw new Error("Missing required field: task");
  }
  
  // Generate child session key
  const childSessionKey = `agent:default:subagent:${crypto.randomUUID()}`;
  
  // If model is provided, patch the session first
  if (input.model) {
    await callGateway("sessions.patch", {
      sessionKey: childSessionKey,
      patch: { model: input.model },
    });
  }
  
  // Spawn the sub-agent
  const result = await callGateway("agent", {
    message: task,
    sessionKey: childSessionKey,
    deliver: false,
    idempotencyKey: crypto.randomUUID(),
    label: input.label,
    agentId: input.agentId,
    thinking: input.thinking,
    runTimeoutSeconds: input.runTimeoutSeconds,
    cleanup: input.cleanup,
  });
  
  return {
    status: "accepted",
    childSessionKey,
    runId: result.runId || result.id,
  };
};
