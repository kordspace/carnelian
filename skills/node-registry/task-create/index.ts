import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TaskCreateParams {
  title: string;
  description?: string;
  dueDate?: string;
  priority?: 'low' | 'medium' | 'high' | 'urgent';
  tags?: string[];
  projectId?: string;
}

export async function execute(
  context: SkillContext,
  params: TaskCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title) {
    return {
      success: false,
      error: 'title is required',
    };
  }

  try {
    const response = await gateway.call('task.create', {
      title: params.title,
      description: params.description || '',
      dueDate: params.dueDate,
      priority: params.priority || 'medium',
      tags: params.tags || [],
      projectId: params.projectId,
      createdAt: new Date().toISOString(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create task',
    };
  }
}
