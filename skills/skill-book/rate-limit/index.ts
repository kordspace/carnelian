import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface RateLimitParams {
  key: string;
  limit: number;
  window?: number;
  strategy?: 'fixed' | 'sliding' | 'token-bucket';
}

export async function execute(
  context: SkillContext,
  params: RateLimitParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key || params.limit === undefined) {
    return {
      success: false,
      error: 'key and limit are required',
    };
  }

  try {
    const response = await gateway.call('rate.limit', {
      key: params.key,
      limit: params.limit,
      window: params.window || 60000,
      strategy: params.strategy || 'fixed',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to check rate limit',
    };
  }
}
