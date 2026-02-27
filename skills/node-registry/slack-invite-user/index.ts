import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SlackInviteUserParams {
  channel: string;
  users: string[];
}

export async function execute(
  context: SkillContext,
  params: SlackInviteUserParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.channel || !params.users || params.users.length === 0) {
    return {
      success: false,
      error: 'channel and users are required',
    };
  }

  try {
    const response = await gateway.call('slack.inviteUser', {
      channel: params.channel,
      users: params.users,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to invite users to Slack channel',
    };
  }
}
