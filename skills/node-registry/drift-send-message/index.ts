import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface DriftSendMessageParams {
  conversationId: string;
  message: string;
  type?: 'chat' | 'private_note';
}

export async function execute(
  context: SkillContext,
  params: DriftSendMessageParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.conversationId || !params.message) {
    return {
      success: false,
      error: 'conversationId and message are required',
    };
  }

  try {
    const response = await gateway.call('drift.sendMessage', {
      conversationId: params.conversationId,
      message: params.message,
      type: params.type || 'chat',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Drift message',
    };
  }
}
