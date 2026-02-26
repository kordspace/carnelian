import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PubSubPublishParams {
  channel: string;
  message: any;
  metadata?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: PubSubPublishParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.channel || params.message === undefined) {
    return {
      success: false,
      error: 'channel and message are required',
    };
  }

  try {
    const response = await gateway.call('pubsub.publish', {
      channel: params.channel,
      message: params.message,
      metadata: params.metadata || {},
      timestamp: Date.now(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to publish to channel',
    };
  }
}
