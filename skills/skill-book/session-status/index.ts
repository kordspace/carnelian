import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SessionStatusParams {
  sessionKey: string;
}

export async function execute(
  context: SkillContext,
  params: SessionStatusParams
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

  try {
    const listResponse = await gateway.call('sessions.list', {
      limit: 1000,
      includeGlobal: true,
      includeUnknown: false,
    });

    const sessions = Array.isArray(listResponse?.sessions) ? listResponse.sessions : [];
    const session = sessions.find((s: any) => s.key === params.sessionKey);

    if (!session) {
      return {
        success: false,
        error: `Session not found: ${params.sessionKey}`,
      };
    }

    const historyResponse = await gateway.call('chat.history', {
      sessionKey: params.sessionKey,
      limit: 1,
    });

    const messages = Array.isArray(historyResponse?.messages) ? historyResponse.messages : [];
    const lastMessage = messages.length > 0 ? messages[messages.length - 1] : null;

    const status = {
      key: params.sessionKey,
      kind: typeof session.kind === 'string' ? session.kind : 'unknown',
      channel: typeof session.channel === 'string' ? session.channel : undefined,
      label: typeof session.label === 'string' ? session.label : undefined,
      displayName: typeof session.displayName === 'string' ? session.displayName : undefined,
      sessionId: typeof session.sessionId === 'string' ? session.sessionId : undefined,
      model: typeof session.model === 'string' ? session.model : undefined,
      contextTokens: typeof session.contextTokens === 'number' ? session.contextTokens : 0,
      totalTokens: typeof session.totalTokens === 'number' ? session.totalTokens : 0,
      thinkingLevel: typeof session.thinkingLevel === 'string' ? session.thinkingLevel : undefined,
      verboseLevel: typeof session.verboseLevel === 'string' ? session.verboseLevel : undefined,
      systemSent: typeof session.systemSent === 'boolean' ? session.systemSent : false,
      abortedLastRun: typeof session.abortedLastRun === 'boolean' ? session.abortedLastRun : false,
      sendPolicy: typeof session.sendPolicy === 'string' ? session.sendPolicy : undefined,
      updatedAt: typeof session.updatedAt === 'number' ? session.updatedAt : undefined,
      createdAt: typeof session.createdAt === 'number' ? session.createdAt : undefined,
      lastMessage: lastMessage ? {
        role: lastMessage.role,
        content: typeof lastMessage.content === 'string' ? lastMessage.content.substring(0, 200) : undefined,
        timestamp: lastMessage.timestamp,
      } : undefined,
    };

    return {
      success: true,
      data: status,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to retrieve session status',
    };
  }
}
