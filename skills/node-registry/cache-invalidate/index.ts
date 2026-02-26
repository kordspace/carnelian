import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CacheInvalidateParams {
  key?: string;
  pattern?: string;
  all?: boolean;
}

export async function execute(
  context: SkillContext,
  params: CacheInvalidateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key && !params.pattern && !params.all) {
    return {
      success: false,
      error: 'key, pattern, or all flag is required',
    };
  }

  try {
    const response = await gateway.call('cache.invalidate', {
      key: params.key,
      pattern: params.pattern,
      all: params.all || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to invalidate cache',
    };
  }
}
