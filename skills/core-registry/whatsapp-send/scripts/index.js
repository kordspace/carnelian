/**
 * whatsapp-send skill wrapper
 * Category: communication
 * Ported from THUMMIM: whatsapp-actions.ts
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: CARNELIAN_GATEWAY_TOKEN
 */

module.exports.run = async (input) => {
  // Parse input
  const action = input.action || "send";
  const to = input.to;
  const content = input.content;
  const messageId = input.messageId;
  const emoji = input.emoji;
  
  if (!to) {
    throw new Error("Missing required field: to");
  }
  
  if (action === "send" && !content) {
    throw new Error("Missing required field: content (required for action 'send')");
  }
  
  if (action === "react" && (!messageId || !emoji)) {
    throw new Error("Missing required fields: messageId and emoji (required for action 'react')");
  }
  
  // Resolve gateway URL and token
  const gatewayUrl = process.env.CARNELIAN_GATEWAY_URL || "http://localhost:18789";
  const token = process.env.CARNELIAN_GATEWAY_TOKEN;
  
  if (!token) {
    throw new Error("CARNELIAN_GATEWAY_TOKEN environment variable is required");
  }
  
  // Build request body
  const body = {
    channel: "whatsapp",
    action,
    target: to,
  };
  
  if (content) {
    body.message = content;
  }
  
  if (messageId) {
    body.messageId = messageId;
  }
  
  if (emoji) {
    body.emoji = emoji;
  }
  
  // Send message with required Authorization header
  const response = await fetch(`${gatewayUrl}/v1/message`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${token}`,
    },
    body: JSON.stringify(body),
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
