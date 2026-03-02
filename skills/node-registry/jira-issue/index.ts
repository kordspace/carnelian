import type { SkillContext, SkillResult } from '../../types';

interface JiraIssueParams {
  action: 'create' | 'update' | 'get' | 'list' | 'transition';
  projectKey?: string;
  issueType?: string;
  summary?: string;
  description?: string;
  issueIdOrKey?: string;
  status?: string;
  priority?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: JiraIssueParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('jira.issue', {
      action: params.action,
      projectKey: params.projectKey,
      issueType: params.issueType,
      summary: params.summary,
      description: params.description,
      issueIdOrKey: params.issueIdOrKey,
      status: params.status,
      priority: params.priority,
      limit: params.limit || 25,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Jira issue action',
    };
  }
}
