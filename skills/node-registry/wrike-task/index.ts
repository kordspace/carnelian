import type { SkillContext, SkillResult } from '../../types';

interface WrikeTaskParams {
  action: 'create' | 'update' | 'get' | 'list' | 'delete';
  folderId?: string;
  taskId?: string;
  title?: string;
  description?: string;
  status?: string;
  assignees?: string[];
  dueDate?: string;
  importance?: 'High' | 'Normal' | 'Low';
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: WrikeTaskParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('wrike.task', {
      action: params.action,
      folderId: params.folderId,
      taskId: params.taskId,
      title: params.title,
      description: params.description,
      status: params.status,
      assignees: params.assignees,
      dueDate: params.dueDate,
      importance: params.importance,
      limit: params.limit || 100,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute Wrike task action' };
  }
}
