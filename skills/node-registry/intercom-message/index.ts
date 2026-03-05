import type { SkillContext, SkillResult } from '../../types';

interface IntercomMessageParams {
  action: 'send_message' | 'create_user' | 'get_conversation' | 'list_conversations';
  userId?: string;
  email?: string;
  message?: string;
  conversationId?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: IntercomMessageParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('intercom.message', {
      action: params.action,
      userId: params.userId,
      email: params.email,
      message: params.message,
      conversationId: params.conversationId,
      limit: params.limit || 20,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Intercom message action',
    };
  }
}
