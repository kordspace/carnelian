import type { SkillContext, SkillResult } from '../../types';

interface IntercomSendMessageParams {
  userId: string;
  message: string;
  from?: 'user' | 'admin';
}

export async function execute(
  context: SkillContext,
  params: IntercomSendMessageParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.userId || !params.message) {
    return {
      success: false,
      error: 'userId and message are required',
    };
  }

  try {
    const response = await gateway.call('intercom.sendMessage', {
      userId: params.userId,
      message: params.message,
      from: params.from || 'admin',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Intercom message',
    };
  }
}
