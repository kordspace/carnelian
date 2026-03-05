import type { SkillContext, SkillResult } from '../../types';

interface SlackReactParams {
  action: 'add' | 'remove' | 'list';
  accountId?: string;
  channelId: string;
  timestamp: string;
  emoji?: string;
}

export async function execute(
  context: SkillContext,
  params: SlackReactParams
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

  if (!params.channelId || !params.timestamp) {
    return {
      success: false,
      error: 'channelId and timestamp are required',
    };
  }

  if ((params.action === 'add' || params.action === 'remove') && !params.emoji) {
    return {
      success: false,
      error: 'emoji is required for add/remove actions',
    };
  }

  try {
    const response = await gateway.call('slack.react', {
      action: params.action,
      accountId: params.accountId,
      channelId: params.channelId,
      timestamp: params.timestamp,
      emoji: params.emoji,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to perform Slack reaction action',
    };
  }
}
