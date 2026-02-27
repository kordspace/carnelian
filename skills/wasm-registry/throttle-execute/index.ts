import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ThrottleExecuteParams {
  key: string;
  interval: number;
  action: string;
  params?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: ThrottleExecuteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key || !params.interval || !params.action) {
    return {
      success: false,
      error: 'key, interval, and action are required',
    };
  }

  try {
    const response = await gateway.call('throttle.execute', {
      key: params.key,
      interval: params.interval,
      action: params.action,
      params: params.params || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to throttle execution',
    };
  }
}
