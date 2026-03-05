import type { SkillContext, SkillResult } from '../../types';

interface RedisSetParams {
  key: string;
  value: string;
  ttl?: number;
}

export async function execute(
  context: SkillContext,
  params: RedisSetParams
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
    const response = await gateway.call('redis.set', {
      key: params.key,
      value: params.value,
      ttl: params.ttl,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to set Redis key',
    };
  }
}
