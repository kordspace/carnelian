import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ClickUpTaskParams {
  action: 'create' | 'update' | 'get' | 'list' | 'delete' | 'move';
  listId?: string;
  taskId?: string;
  name?: string;
  description?: string;
  assignees?: string[];
  status?: string;
  priority?: number;
  dueDate?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: ClickUpTaskParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('clickup.task', {
      action: params.action,
      listId: params.listId,
      taskId: params.taskId,
      name: params.name,
      description: params.description,
      assignees: params.assignees,
      status: params.status,
      priority: params.priority,
      dueDate: params.dueDate,
      limit: params.limit || 100,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute ClickUp task action' };
  }
}
