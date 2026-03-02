import type { SkillContext, SkillResult } from '../../types';

interface QueuePopParams {
  queue: string;
  timeout?: number;
  count?: number;
}

export async function execute(
  context: SkillContext,
  params: QueuePopParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.queue) {
    return {
      success: false,
      error: 'queue is required',
    };
  }

  try {
    const response = await gateway.call('queue.pop', {
      queue: params.queue,
      timeout: params.timeout || 0,
      count: params.count || 1,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to pop from queue',
    };
  }
}
