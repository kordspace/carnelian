import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SignalSendParams {
  recipient: string;
  message: string;
  attachments?: string[];
}

export async function execute(
  context: SkillContext,
  params: SignalSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.recipient || !params.message) {
    return {
      success: false,
      error: 'recipient and message are required',
    };
  }

  try {
    const response = await gateway.call('signal.send', {
      recipient: params.recipient,
      message: params.message,
      attachments: params.attachments || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Signal message',
    };
  }
}
