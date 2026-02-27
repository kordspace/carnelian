/**
 * canvas-render skill wrapper
 * Category: creative
 * Ported from THUMMIM: canvas-tool.ts
 *
 * Sandbox globals available: fetch, crypto, process.env
 * Required env vars: CARNELIAN_GATEWAY_URL
 */

module.exports.run = async (input) => {
  // Validate input
  const action = input.action;
  
  if (!action) {
    throw new Error("Missing required field: action");
  }
  
  // Resolve gateway URL and token
  const gatewayUrl = input.gatewayUrl || process.env.CARNELIAN_GATEWAY_URL;
  if (!gatewayUrl) {
    throw new Error("CARNELIAN_GATEWAY_URL environment variable is not set and gatewayUrl not provided");
  }
  
  const gatewayToken = input.gatewayToken || process.env.CARNELIAN_GATEWAY_TOKEN;
  
  // Helper: Call gateway node.invoke endpoint
  const callGateway = async (command, params) => {
    const headers = {
      "Content-Type": "application/json",
    };
    
    if (gatewayToken) {
      headers["Authorization"] = `Bearer ${gatewayToken}`;
    }
    
    const response = await fetch(`${gatewayUrl}/api/tool/node.invoke`, {
      method: "POST",
      headers,
      body: JSON.stringify({
        nodeId: input.node,
        command,
        params,
        idempotencyKey: crypto.randomUUID(),
      }),
    });
    
    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Gateway error (${response.status}): ${errorText}`);
    }
    
    return await response.json();
  };
  
  // Execute action
  switch (action) {
    case "present": {
      const params = {
        target: input.target,
        x: input.x,
        y: input.y,
        width: input.width,
        height: input.height,
      };
      const result = await callGateway("present", params);
      return { ok: true, result };
    }
    
    case "hide": {
      const result = await callGateway("hide", {});
      return { ok: true, result };
    }
    
    case "navigate": {
      if (!input.url) {
        throw new Error("Missing required field for navigate action: url");
      }
      const result = await callGateway("navigate", { url: input.url });
      return { ok: true, result };
    }
    
    case "eval": {
      if (!input.javaScript) {
        throw new Error("Missing required field for eval action: javaScript");
      }
      const result = await callGateway("eval", { javaScript: input.javaScript });
      return { ok: true, result };
    }
    
    case "snapshot": {
      const params = {
        outputFormat: input.outputFormat || "png",
        maxWidth: input.maxWidth,
        quality: input.quality,
        delayMs: input.delayMs,
      };
      const result = await callGateway("snapshot", params);
      
      // Return snapshot data
      return {
        base64: result.base64 || result.data,
        format: params.outputFormat,
      };
    }
    
    case "a2ui_push": {
      if (!input.jsonl) {
        throw new Error("Missing required field for a2ui_push action: jsonl");
      }
      const result = await callGateway("a2ui_push", { jsonl: input.jsonl });
      return { ok: true, result };
    }
    
    case "a2ui_reset": {
      const result = await callGateway("a2ui_reset", {});
      return { ok: true, result };
    }
    
    default:
      throw new Error(`Unknown action: ${action}`);
  }
};
