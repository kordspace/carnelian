import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface RedisGetParams {
  key: string;
}

export async function execute(
  context: SkillContext,
  params: RedisGetParams
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
    const response = await gateway.call('redis.get', {
      key: params.key,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get Redis key',
    };
  }
}
