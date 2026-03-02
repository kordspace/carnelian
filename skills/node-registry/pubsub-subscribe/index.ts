import type { SkillContext, SkillResult } from '../../types';

interface PubSubSubscribeParams {
  channel: string;
  handler?: string;
  filter?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: PubSubSubscribeParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.channel) {
    return {
      success: false,
      error: 'channel is required',
    };
  }

  try {
    const response = await gateway.call('pubsub.subscribe', {
      channel: params.channel,
      handler: params.handler,
      filter: params.filter || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to subscribe to channel',
    };
  }
}
