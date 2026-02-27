/**
 * agent-step skill wrapper
 * Category: automation
 * Ported from THUMMIM: agent-step-tool.ts
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
  const sessionKey = input.sessionKey;
  const message = input.message;
  const extraSystemPrompt = input.extraSystemPrompt;
  
  if (!sessionKey) {
    throw new Error("Missing required field: sessionKey");
  }
  
  if (!message) {
    throw new Error("Missing required field: message");
  }
  
  if (!extraSystemPrompt) {
    throw new Error("Missing required field: extraSystemPrompt");
  }
  
  // Generate idempotency key
  const stepIdem = crypto.randomUUID();
  
  // Call agent method
  const agentResult = await callGateway("agent", {
    message,
    sessionKey,
    idempotencyKey: stepIdem,
    deliver: false,
    channel: input.channel || "internal",
    lane: input.lane || "nested",
    extraSystemPrompt,
  });
  
  const runId = agentResult.runId || agentResult.id || stepIdem;
  
  // Wait for completion
  const timeoutMs = Math.min(input.timeoutMs || 30000, 60000);
  const waitResult = await callGateway("agent.wait", {
    runId,
    timeoutMs,
  }, { timeoutMs: timeoutMs + 2000 });
  
  // Check wait status
  if (waitResult.status !== "ok") {
    return {
      ok: false,
      status: waitResult.status,
    };
  }
  
  // Read chat history to get the latest assistant reply
  const history = await callGateway("chat.history", {
    sessionKey,
    limit: 50,
  });
  
  // Filter out tool messages and find the last assistant message
  const messages = history.messages || [];
  
  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    
    // Skip tool messages
    if (msg.role === "tool") continue;
    
    // Skip messages with only tool-result blocks
    if (Array.isArray(msg.content)) {
      const hasOnlyToolResults = msg.content.every(block => 
        block.type === "tool_result" || block.type === "tool_use"
      );
      if (hasOnlyToolResults) continue;
    }
    
    // Found assistant message
    if (msg.role === "assistant") {
      let replyText = "";
      
      if (typeof msg.content === "string") {
        replyText = msg.content;
      } else if (Array.isArray(msg.content)) {
        // Extract text blocks
        replyText = msg.content
          .filter(block => block.type === "text")
          .map(block => block.text)
          .join("");
      }
      
      return {
        ok: true,
        reply: replyText,
      };
    }
  }
  
  // No assistant message found
  return {
    ok: false,
    status: "no_reply",
  };
};
