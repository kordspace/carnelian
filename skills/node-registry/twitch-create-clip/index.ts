import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TwitchCreateClipParams {
  broadcasterId: string;
  hasDelay?: boolean;
}

export async function execute(
  context: SkillContext,
  params: TwitchCreateClipParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.broadcasterId) {
    return {
      success: false,
      error: 'broadcasterId is required',
    };
  }

  try {
    const response = await gateway.call('twitch.createClip', {
      broadcasterId: params.broadcasterId,
      hasDelay: params.hasDelay || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Twitch clip',
    };
  }
}
