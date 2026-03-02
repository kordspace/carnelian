import type { SkillContext, SkillResult } from '../../types';

interface BasecampCreateParams {
  projectId: string;
  type: 'todo' | 'message' | 'document';
  title: string;
  content?: string;
  assignees?: string[];
  dueDate?: string;
}

export async function execute(
  context: SkillContext,
  params: BasecampCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.projectId || !params.type || !params.title) {
    return {
      success: false,
      error: 'projectId, type, and title are required',
    };
  }

  try {
    const response = await gateway.call('basecamp.create', {
      projectId: params.projectId,
      type: params.type,
      title: params.title,
      content: params.content,
      assignees: params.assignees,
      dueDate: params.dueDate,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Basecamp item',
    };
  }
}
