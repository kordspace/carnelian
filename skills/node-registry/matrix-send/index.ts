import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface MatrixSendParams {
  roomId: string;
  message: string;
  msgtype?: 'text' | 'notice' | 'emote';
}

export async function execute(
  context: SkillContext,
  params: MatrixSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.roomId || !params.message) {
    return {
      success: false,
      error: 'roomId and message are required',
    };
  }

  try {
    const response = await gateway.call('matrix.send', {
      roomId: params.roomId,
      message: params.message,
      msgtype: params.msgtype || 'text',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Matrix message',
    };
  }
}
