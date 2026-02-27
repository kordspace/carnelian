import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface QueuePublishParams {
  queue: string;
  message: any;
  priority?: number;
  delay?: number;
}

export async function execute(
  context: SkillContext,
  params: QueuePublishParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.queue || params.message === undefined) {
    return {
      success: false,
      error: 'queue and message are required',
    };
  }

  try {
    const response = await gateway.call('queue.publish', {
      queue: params.queue,
      message: params.message,
      priority: params.priority || 0,
      delay: params.delay || 0,
      timestamp: Date.now(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to publish to queue',
    };
  }
}
