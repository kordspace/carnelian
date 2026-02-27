import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SlackChannelParams {
  action: 'list' | 'create' | 'archive' | 'info';
  accountId?: string;
  channelId?: string;
  name?: string;
  isPrivate?: boolean;
}

export async function execute(
  context: SkillContext,
  params: SlackChannelParams
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
    const response = await gateway.call('slack.channel', {
      action: params.action,
      accountId: params.accountId,
      channelId: params.channelId,
      name: params.name,
      isPrivate: params.isPrivate,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to perform Slack channel action',
    };
  }
}
