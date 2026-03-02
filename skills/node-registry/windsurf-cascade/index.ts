import type { SkillContext, SkillResult } from '../../types';
import { writeFileSync, readFileSync, existsSync, appendFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

interface WindsurfCascadeParams {
  action: 'message' | 'delegate' | 'status' | 'request_help' | 'share_context';
  content?: string;
  task?: string;
  priority?: 'low' | 'medium' | 'high' | 'urgent';
  context?: string;
  files?: string[];
  wait?: boolean;
}

interface CascadeMessage {
  id: string;
  type: 'carnelian_to_cascade' | 'cascade_to_carnelian';
  message?: string;
  response?: string;
  replyTo?: string;
  timestamp: string;
  wait?: boolean;
}

const CARNELIAN_HOME = process.env.CARNELIAN_HOME || `${process.env.HOME || process.env.USERPROFILE}/.carnelian`;
const CASCADE_CHANNEL_PATH = `${CARNELIAN_HOME}/cascade-channel.jsonl`;
const CASCADE_RESPONSES_PATH = `${CARNELIAN_HOME}/cascade-responses.jsonl`;

async function sendToCascade(message: string, wait = true): Promise<string> {
  const messageId = `msg_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

  try {
    const dir = dirname(CASCADE_CHANNEL_PATH);
    if (!existsSync(dir)) {
      mkdirSync(dir, { recursive: true });
    }

    const entry: CascadeMessage = {
      id: messageId,
      type: 'carnelian_to_cascade',
      message,
      timestamp: new Date().toISOString(),
      wait,
    };

    appendFileSync(CASCADE_CHANNEL_PATH, JSON.stringify(entry) + '\n');

    if (!wait) {
      return `Message queued for Cascade (id: ${messageId}). Cascade will process this asynchronously.`;
    }

    // Poll for response
    const maxWaitMs = 120000; // 2 minutes
    const pollIntervalMs = 2000;
    const startTime = Date.now();

    while (Date.now() - startTime < maxWaitMs) {
      await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));

      if (existsSync(CASCADE_RESPONSES_PATH)) {
        const content = readFileSync(CASCADE_RESPONSES_PATH, 'utf-8');
        const lines = content.trim().split('\n').filter(Boolean);

        for (const line of lines.reverse()) {
          try {
            const entry = JSON.parse(line) as CascadeMessage;
            if (entry.replyTo === messageId && entry.type === 'cascade_to_carnelian') {
              return entry.response || 'Response received';
            }
          } catch {
            continue;
          }
        }
      }
    }

    return `Message sent to Cascade but response timed out after ${maxWaitMs / 1000}s. Message id: ${messageId}`;
  } catch (error) {
    return `Error communicating with Cascade: ${error instanceof Error ? error.message : String(error)}`;
  }
}

export async function execute(
  context: SkillContext,
  params: WindsurfCascadeParams
): Promise<SkillResult> {
  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  const wait = params.wait !== false;

  try {
    if (params.action === 'status') {
      return {
        success: true,
        data: {
          channelPath: CASCADE_CHANNEL_PATH,
          responsesPath: CASCADE_RESPONSES_PATH,
          status: 'ready',
        },
      };
    }

    if (params.action === 'message') {
      if (!params.content) {
        return {
          success: false,
          error: 'content is required for message action',
        };
      }
      const response = await sendToCascade(params.content, wait);
      return {
        success: true,
        data: { response },
      };
    }

    if (params.action === 'delegate') {
      if (!params.task) {
        return {
          success: false,
          error: 'task is required for delegate action',
        };
      }
      const priority = params.priority || 'medium';
      const delegateMessage = `[TASK DELEGATION - Priority: ${priority.toUpperCase()}]\n\n${params.task}\n\nPlease work on this task and report back when complete.`;
      const response = await sendToCascade(delegateMessage, wait);
      return {
        success: true,
        data: { taskDelegated: true, priority, response },
      };
    }

    if (params.action === 'request_help') {
      if (!params.content) {
        return {
          success: false,
          error: 'content is required for request_help action',
        };
      }
      const helpMessage = `[HELP REQUEST]\n\n${params.content}\n\nPlease assist with this request.`;
      const response = await sendToCascade(helpMessage, wait);
      return {
        success: true,
        data: { helpRequested: true, response },
      };
    }

    if (params.action === 'share_context') {
      let contextMessage = '[CONTEXT SHARE]\n\n';
      if (params.context) {
        contextMessage += `Context:\n${params.context}\n\n`;
      }
      if (params.files && params.files.length > 0) {
        contextMessage += `Relevant files:\n${params.files.map((f) => `- ${f}`).join('\n')}\n`;
      }
      const response = await sendToCascade(contextMessage, wait);
      return {
        success: true,
        data: { contextShared: true, response },
      };
    }

    return {
      success: false,
      error: `Unknown action: ${params.action}`,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Windsurf Cascade action',
    };
  }
}
