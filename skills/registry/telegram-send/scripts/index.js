/**
 * telegram-send skill wrapper
 * Category: communication
 * Ported from THUMMIM: telegram-actions.ts
 *
 * Sandbox globals available: fetch, URL, URLSearchParams, process.env
 * Required env vars: TELEGRAM_BOT_TOKEN
 */

module.exports.run = async (input) => {
  // Parse input
  const to = input.to;
  const content = input.content;
  const replyToMessageId = input.replyToMessageId;
  const messageThreadId = input.messageThreadId;
  const silent = input.silent;
  
  if (!to) {
    throw new Error("Missing required field: to");
  }
  
  if (!content) {
    throw new Error("Missing required field: content");
  }
  
  // Resolve token
  const token = process.env.TELEGRAM_BOT_TOKEN;
  if (!token) {
    throw new Error("TELEGRAM_BOT_TOKEN environment variable is not set");
  }
  
  // Build request body
  const body = {
    chat_id: to,
    text: content,
    parse_mode: "HTML",
  };
  
  if (replyToMessageId) {
    body.reply_parameters = {
      message_id: replyToMessageId,
    };
  }
  
  if (messageThreadId) {
    body.message_thread_id = messageThreadId;
  }
  
  if (silent) {
    body.disable_notification = true;
  }
  
  // Send message
  const response = await fetch(`https://api.telegram.org/bot${token}/sendMessage`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Telegram API error (${response.status}): ${errorText}`);
  }
  
  const data = await response.json();
  
  if (!data.ok) {
    throw new Error(`Telegram API error: ${data.description || "Unknown error"}`);
  }
  
  return {
    ok: true,
    messageId: data.result.message_id,
    chatId: data.result.chat.id,
  };
};
