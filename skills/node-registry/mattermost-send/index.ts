import type { SkillContext, SkillResult } from '../../types';

interface MattermostSendParams {
  channelId: string;
  message: string;
  rootId?: string;
}

export async function execute(
  context: SkillContext,
  params: MattermostSendParams
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
    const response = await gateway.call('mattermost.send', {
      channelId: params.channelId,
      message: params.message,
      rootId: params.rootId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Mattermost message',
    };
  }
}
