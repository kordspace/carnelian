import type { SkillContext, SkillResult } from '../../types';

interface RateLimitCheckParams {
  key: string;
  limit: number;
  window?: number;
}

export async function execute(
  context: SkillContext,
  params: RateLimitCheckParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key || !params.limit) {
    return {
      success: false,
      error: 'key and limit are required',
    };
  }

  try {
    const response = await gateway.call('ratelimit.check', {
      key: params.key,
      limit: params.limit,
      window: params.window || 60000,
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
