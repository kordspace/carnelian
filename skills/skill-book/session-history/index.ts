import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SessionHistoryParams {
  sessionKey: string;
  limit?: number;
  offset?: number;
  includeSystem?: boolean;
}

export async function execute(
  context: SkillContext,
  params: SessionHistoryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.sessionKey || typeof params.sessionKey !== 'string') {
    return {
      success: false,
      error: 'sessionKey is required',
    };
  }

  const limit = params.limit && params.limit > 0 ? Math.floor(params.limit) : 50;
  const offset = params.offset && params.offset >= 0 ? Math.floor(params.offset) : 0;

  try {
    const response = await gateway.call('chat.history', {
      sessionKey: params.sessionKey,
      limit,
      offset,
    });

    let messages = Array.isArray(response?.messages) ? response.messages : [];

    if (!params.includeSystem) {
      messages = messages.filter((msg: any) => {
        const role = typeof msg?.role === 'string' ? msg.role : '';
        return role !== 'system';
      });
    }

    return {
      success: true,
      data: {
        sessionKey: params.sessionKey,
        count: messages.length,
        offset,
        messages,
      },
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to retrieve session history',
    };
  }
}
