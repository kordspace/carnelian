import type { SkillContext, SkillResult } from '../../types';

interface LinearIssueParams {
  action: 'create' | 'update' | 'get' | 'list' | 'delete';
  teamId?: string;
  issueId?: string;
  title?: string;
  description?: string;
  stateId?: string;
  priority?: number;
  assigneeId?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: LinearIssueParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('linear.issue', {
      action: params.action,
      teamId: params.teamId,
      issueId: params.issueId,
      title: params.title,
      description: params.description,
      stateId: params.stateId,
      priority: params.priority,
      assigneeId: params.assigneeId,
      limit: params.limit || 50,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute Linear issue action' };
  }
}
