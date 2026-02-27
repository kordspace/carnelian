/**
 * slack-send skill wrapper
 * Category: communication
 * Ported from THUMMIM: slack-actions.ts
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: SLACK_BOT_TOKEN
 */

module.exports.run = async (input) => {
  // Parse input
  const to = input.to;
  const content = input.content;
  const threadTs = input.threadTs;
  const mediaUrl = input.mediaUrl;
  
  if (!to) {
    throw new Error("Missing required field: to");
  }
  
  if (!content) {
    throw new Error("Missing required field: content");
  }
  
  // Resolve token
  const token = process.env.SLACK_BOT_TOKEN;
  if (!token) {
    throw new Error("SLACK_BOT_TOKEN environment variable is not set");
  }
  
  // Build request body
  const body = {
    channel: to,
    text: content,
  };
  
  if (threadTs) {
    body.thread_ts = threadTs;
  }
  
  if (mediaUrl) {
    body.attachments = [
      {
        image_url: mediaUrl,
      },
    ];
  }
  
  // Send message
  const response = await fetch("https://slack.com/api/chat.postMessage", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${token}`,
    },
    body: JSON.stringify(body),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Slack API error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  
  if (!data.ok) {
    throw new Error(`Slack API error: ${data.error || "Unknown error"}`);
  }
  
  return {
    ok: true,
    ts: data.ts,
    channel: data.channel,
  };
};
