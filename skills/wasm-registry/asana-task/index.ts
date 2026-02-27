import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AsanaTaskParams {
  action: 'create' | 'update' | 'get' | 'list' | 'complete' | 'delete';
  projectId?: string;
  taskId?: string;
  name?: string;
  notes?: string;
  assignee?: string;
  dueOn?: string;
  completed?: boolean;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: AsanaTaskParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('asana.task', {
      action: params.action,
      projectId: params.projectId,
      taskId: params.taskId,
      name: params.name,
      notes: params.notes,
      assignee: params.assignee,
      dueOn: params.dueOn,
      completed: params.completed,
      limit: params.limit || 50,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Asana task action',
    };
  }
}
