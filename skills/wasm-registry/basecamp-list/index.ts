import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BasecampListParams {
  projectId: string;
  type?: 'todos' | 'messages' | 'documents' | 'all';
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: BasecampListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.projectId) {
    return {
      success: false,
      error: 'projectId is required',
    };
  }

  try {
    const response = await gateway.call('basecamp.list', {
      projectId: params.projectId,
      type: params.type || 'all',
      limit: params.limit || 50,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list Basecamp items',
    };
  }
}
