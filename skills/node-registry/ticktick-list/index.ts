import type { SkillContext, SkillResult } from '../../types';

interface TickTickListParams {
  listId?: string;
  status?: 'active' | 'completed' | 'all';
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: TickTickListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('ticktick.list', {
      listId: params.listId,
      status: params.status || 'active',
      limit: params.limit || 100,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list TickTick tasks',
    };
  }
}
