import type { SkillContext, SkillResult } from '../../types';

interface LinearCreateIssueParams {
  teamId: string;
  title: string;
  description?: string;
  priority?: number;
  assigneeId?: string;
  labelIds?: string[];
}

export async function execute(
  context: SkillContext,
  params: LinearCreateIssueParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.teamId || !params.title) {
    return {
      success: false,
      error: 'teamId and title are required',
    };
  }

  try {
    const response = await gateway.call('linear.createIssue', {
      teamId: params.teamId,
      title: params.title,
      description: params.description || '',
      priority: params.priority || 0,
      assigneeId: params.assigneeId,
      labelIds: params.labelIds || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Linear issue',
    };
  }
}
