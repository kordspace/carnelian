import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface DebounceExecuteParams {
  key: string;
  delay: number;
  action: string;
  params?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: DebounceExecuteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key || !params.delay || !params.action) {
    return {
      success: false,
      error: 'key, delay, and action are required',
    };
  }

  try {
    const response = await gateway.call('debounce.execute', {
      key: params.key,
      delay: params.delay,
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
      error: error instanceof Error ? error.message : 'Failed to debounce execution',
    };
  }
}
