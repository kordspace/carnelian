import type { SkillContext, SkillResult } from '../../types';

interface QueuePushParams {
  queue: string;
  message: unknown;
  priority?: number;
  delay?: number;
}

export async function execute(
  context: SkillContext,
  params: QueuePushParams
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
    const response = await gateway.call('queue.push', {
      queue: params.queue,
      message: params.message,
      priority: params.priority || 0,
      delay: params.delay || 0,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to push to queue',
    };
  }
}
