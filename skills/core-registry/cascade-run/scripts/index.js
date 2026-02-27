/**
 * cascade-run skill wrapper
 * Category: automation
 * Ported from THUMMIM: cascade-tool.ts
 *
 * Sandbox globals available: fs, crypto, process.env
 * Required env vars: none (optional OPENCLAW_HOME)
 */

// In-memory connection state
const connectionState = {
  pendingMessages: 0,
  lastMessage: null,
  lastResponse: null,
};

// Helper: Join paths (simple cross-platform implementation)
function joinPath(dir, file) {
  const separator = dir.includes('\\') ? '\\' : '/';
  return dir.endsWith(separator) ? dir + file : dir + separator + file;
}

module.exports.run = async (input) => {
  // Validate input
  const action = input.action;
  
  if (!action) {
    throw new Error("Missing required field: action");
  }
  
  // Resolve channel directory
  const channelDir = process.env.OPENCLAW_HOME || 
                     (process.env.HOME ? `${process.env.HOME}/.openclaw` : null) ||
                     (process.env.USERPROFILE ? `${process.env.USERPROFILE}/.openclaw` : null);
  
  if (!channelDir) {
    throw new Error("Cannot resolve channel directory. Set OPENCLAW_HOME, HOME, or USERPROFILE environment variable.");
  }
  
  const channelPath = joinPath(channelDir, "cascade-channel.jsonl");
  const responsePath = joinPath(channelDir, "cascade-responses.jsonl");
  
  // Execute action
  switch (action) {
    case "status":
      return {
        pendingMessages: connectionState.pendingMessages,
        lastMessage: connectionState.lastMessage,
        lastResponse: connectionState.lastResponse,
      };
    
    case "message":
    case "delegate":
    case "request_help":
    case "share_context": {
      if (!input.text) {
        throw new Error("Missing required field: text");
      }
      
      // Format message based on action
      let messageText;
      if (action === "message") {
        messageText = input.text;
      } else if (action === "delegate") {
        messageText = `[DELEGATE] ${input.text}`;
      } else if (action === "request_help") {
        messageText = `[REQUEST_HELP] ${input.text}`;
      } else if (action === "share_context") {
        messageText = `[SHARE_CONTEXT] ${input.text}`;
      }
      
      // Generate message ID
      const messageId = crypto.randomUUID();
      
      // Build JSONL entry
      const entry = {
        type: "mim_to_cascade",
        messageId,
        text: messageText,
        timestamp: new Date().toISOString(),
      };
      
      // Append to channel file
      await fs.appendFile(channelPath, JSON.stringify(entry) + "\n", "utf-8");
      
      // Update connection state
      connectionState.pendingMessages++;
      connectionState.lastMessage = messageText;
      
      // Wait for response if requested
      const wait = input.wait !== false;
      
      if (!wait) {
        return {
          ok: true,
          queued: true,
          messageId,
        };
      }
      
      // Poll for response (up to 120 seconds)
      const maxAttempts = 60; // 60 * 2s = 120s
      let attempts = 0;
      
      while (attempts < maxAttempts) {
        await new Promise(resolve => setTimeout(resolve, 2000));
        attempts++;
        
        // Read response file
        let responseContent;
        try {
          responseContent = await fs.readFile(responsePath, "utf-8");
        } catch (err) {
          // File doesn't exist yet, continue polling
          continue;
        }
        
        // Parse JSONL and look for matching response
        const lines = responseContent.split("\n").filter(line => line.trim());
        
        for (const line of lines) {
          try {
            const responseEntry = JSON.parse(line);
            
            if (responseEntry.replyTo === messageId && responseEntry.type === "cascade_to_mim") {
              // Found matching response
              connectionState.pendingMessages = Math.max(0, connectionState.pendingMessages - 1);
              connectionState.lastResponse = responseEntry.response;
              
              return {
                ok: true,
                response: responseEntry.response,
              };
            }
          } catch (parseErr) {
            // Skip invalid JSON lines
            continue;
          }
        }
      }
      
      // Timeout after 120 seconds
      return {
        ok: true,
        response: "Timeout: No response from Cascade after 120 seconds. The message was queued but may still be processed.",
        messageId,
      };
    }
    
    default:
      throw new Error(`Unknown action: ${action}`);
  }
};
