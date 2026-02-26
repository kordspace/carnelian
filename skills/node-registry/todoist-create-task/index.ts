import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TodoistCreateTaskParams {
  content: string;
  description?: string;
  projectId?: string;
  dueString?: string;
  priority?: number;
  labels?: string[];
}

export async function execute(
  context: SkillContext,
  params: TodoistCreateTaskParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.content) {
    return {
      success: false,
      error: 'content is required',
    };
  }

  try {
    const response = await gateway.call('todoist.createTask', {
      content: params.content,
      description: params.description || '',
      projectId: params.projectId,
      dueString: params.dueString,
      priority: params.priority || 1,
      labels: params.labels || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Todoist task',
    };
  }
}
