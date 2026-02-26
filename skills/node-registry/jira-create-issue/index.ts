import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface JiraCreateIssueParams {
  project: string;
  summary: string;
  description?: string;
  issueType?: string;
  priority?: string;
  assignee?: string;
}

export async function execute(
  context: SkillContext,
  params: JiraCreateIssueParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.project || !params.summary) {
    return {
      success: false,
      error: 'project and summary are required',
    };
  }

  try {
    const response = await gateway.call('jira.createIssue', {
      project: params.project,
      summary: params.summary,
      description: params.description || '',
      issueType: params.issueType || 'Task',
      priority: params.priority || 'Medium',
      assignee: params.assignee,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Jira issue',
    };
  }
}
