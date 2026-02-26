import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CacheGetParams {
  key: string;
  namespace?: string;
}

export async function execute(
  context: SkillContext,
  params: CacheGetParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key) {
    return {
      success: false,
      error: 'key is required',
    };
  }

  try {
    const response = await gateway.call('cache.get', {
      key: params.key,
      namespace: params.namespace || 'default',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get cache value',
    };
  }
}
