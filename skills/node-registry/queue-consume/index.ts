import type { SkillContext, SkillResult } from '../../types';

interface QueueConsumeParams {
  queue: string;
  count?: number;
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: QueueConsumeParams
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
    const response = await gateway.call('queue.consume', {
      queue: params.queue,
      count: params.count || 1,
      timeout: params.timeout || 5000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to consume from queue',
    };
  }
}
