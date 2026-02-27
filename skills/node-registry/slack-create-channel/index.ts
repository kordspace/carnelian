import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SlackCreateChannelParams {
  name: string;
  isPrivate?: boolean;
  description?: string;
}

export async function execute(
  context: SkillContext,
  params: SlackCreateChannelParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name) {
    return {
      success: false,
      error: 'name is required',
    };
  }

  try {
    const response = await gateway.call('slack.createChannel', {
      name: params.name,
      isPrivate: params.isPrivate || false,
      description: params.description,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Slack channel',
    };
  }
}
