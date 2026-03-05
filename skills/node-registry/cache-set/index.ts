import type { SkillContext, SkillResult } from '../../types';

interface CacheSetParams {
  key: string;
  value: unknown;
  namespace?: string;
  ttl?: number;
}

export async function execute(
  context: SkillContext,
  params: CacheSetParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key || params.value === undefined) {
    return {
      success: false,
      error: 'key and value are required',
    };
  }

  try {
    const response = await gateway.call('cache.set', {
      key: params.key,
      value: params.value,
      namespace: params.namespace || 'default',
      ttl: params.ttl,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to set cache value',
    };
  }
}
