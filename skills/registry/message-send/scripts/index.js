/**
 * message-send skill wrapper
 * Category: communication
 * Ported from THUMMIM: message-tool.ts
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: CARNELIAN_GATEWAY_TOKEN
 */

module.exports.run = async (input) => {
  // Parse input - pass through the full input object
  const action = input.action;
  
  if (!action) {
    throw new Error("Missing required field: action");
  }
  
  // Resolve gateway URL and token
  const gatewayUrl = process.env.CARNELIAN_GATEWAY_URL || "http://localhost:18789";
  const token = process.env.CARNELIAN_GATEWAY_TOKEN;
  
  if (!token) {
    throw new Error("CARNELIAN_GATEWAY_TOKEN environment variable is required");
  }
  
  // Send message with full input as body and required Authorization header
  const response = await fetch(`${gatewayUrl}/v1/message`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${token}`,
    },
    body: JSON.stringify(input),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Carnelian gateway error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  
  return {
    ok: true,
    result: data,
  };
};
