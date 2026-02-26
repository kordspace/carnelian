import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SessionListParams {
  kinds?: string[];
  limit?: number;
  activeMinutes?: number;
  messageLimit?: number;
}

interface SessionRow {
  key: string;
  kind: string;
  channel?: string;
  label?: string;
  displayName?: string;
  updatedAt?: number;
  sessionId?: string;
  model?: string;
  contextTokens?: number;
  totalTokens?: number;
  messages?: unknown[];
}

export async function execute(
  context: SkillContext,
  params: SessionListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  const limit = params.limit && params.limit > 0 ? Math.floor(params.limit) : undefined;
  const activeMinutes = params.activeMinutes && params.activeMinutes > 0 
    ? Math.floor(params.activeMinutes) 
    : undefined;
  const messageLimit = params.messageLimit !== undefined 
    ? Math.min(Math.max(0, Math.floor(params.messageLimit)), 20) 
    : 0;

  try {
    const response = await gateway.call('sessions.list', {
      limit,
      activeMinutes,
      includeGlobal: true,
      includeUnknown: false,
    });

    const sessions = Array.isArray(response?.sessions) ? response.sessions : [];
    const allowedKinds = params.kinds ? new Set(params.kinds.map(k => k.toLowerCase())) : undefined;

    const rows: SessionRow[] = [];

    for (const entry of sessions) {
      if (!entry || typeof entry !== 'object') continue;
      const key = typeof entry.key === 'string' ? entry.key : '';
      if (!key || key === 'unknown') continue;

      const kind = typeof entry.kind === 'string' ? entry.kind : 'other';
      if (allowedKinds && !allowedKinds.has(kind)) continue;

      const row: SessionRow = {
        key,
        kind,
        channel: typeof entry.channel === 'string' ? entry.channel : undefined,
        label: typeof entry.label === 'string' ? entry.label : undefined,
        displayName: typeof entry.displayName === 'string' ? entry.displayName : undefined,
        updatedAt: typeof entry.updatedAt === 'number' ? entry.updatedAt : undefined,
        sessionId: typeof entry.sessionId === 'string' ? entry.sessionId : undefined,
        model: typeof entry.model === 'string' ? entry.model : undefined,
        contextTokens: typeof entry.contextTokens === 'number' ? entry.contextTokens : undefined,
        totalTokens: typeof entry.totalTokens === 'number' ? entry.totalTokens : undefined,
      };

      if (messageLimit > 0) {
        const history = await gateway.call('chat.history', {
          sessionKey: key,
          limit: messageLimit,
        });
        const messages = Array.isArray(history?.messages) ? history.messages : [];
        row.messages = messages.slice(-messageLimit);
      }

      rows.push(row);
    }

    return {
      success: true,
      data: {
        count: rows.length,
        sessions: rows,
      },
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list sessions',
    };
  }
}
