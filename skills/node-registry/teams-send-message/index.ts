import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TeamsSendMessageParams {
  channelId: string;
  message: string;
  mentions?: string[];
}

export async function execute(
  context: SkillContext,
  params: TeamsSendMessageParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.channelId || !params.message) {
    return {
      success: false,
      error: 'channelId and message are required',
    };
  }

  try {
    const response = await gateway.call('teams.sendMessage', {
      channelId: params.channelId,
      message: params.message,
      mentions: params.mentions || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Teams message',
    };
  }
}
