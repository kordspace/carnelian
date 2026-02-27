/**
 * discord-send skill wrapper
 * Category: communication
 * Ported from THUMMIM: discord-actions.ts
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: DISCORD_TOKEN
 */

module.exports.run = async (input) => {
  // Parse input
  const channelId = input.channelId;
  const content = input.content;
  const replyToMessageId = input.replyToMessageId;
  
  if (!channelId) {
    throw new Error("Missing required field: channelId");
  }
  
  if (!content) {
    throw new Error("Missing required field: content");
  }
  
  // Resolve token
  const token = process.env.DISCORD_TOKEN;
  if (!token) {
    throw new Error("DISCORD_TOKEN environment variable is not set");
  }
  
  // Build request body
  const body = {
    content,
  };
  
  if (replyToMessageId) {
    body.message_reference = {
      message_id: replyToMessageId,
    };
  }
  
  // Send message
  const response = await fetch(`https://discord.com/api/v10/channels/${channelId}/messages`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bot ${token}`,
    },
    body: JSON.stringify(body),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Discord API error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  
  return {
    ok: true,
    messageId: data.id,
    channelId: data.channel_id,
  };
};
