import type { SkillContext, SkillResult } from '../../types';

interface TaskListParams {
  status?: 'all' | 'active' | 'completed' | 'pending';
  projectId?: string;
  tags?: string[];
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: TaskListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('task.list', {
      status: params.status || 'all',
      projectId: params.projectId,
      tags: params.tags || [],
      limit: params.limit || 100,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list tasks',
    };
  }
}
