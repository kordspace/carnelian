import type { SkillContext, SkillResult } from '../../types';

interface MemoryReadParams {
  query?: string;
  key?: string;
  tags?: string[];
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: MemoryReadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('memory.read', {
      query: params.query,
      key: params.key,
      tags: params.tags || [],
      limit: params.limit || 10,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to read memory',
    };
  }
}
