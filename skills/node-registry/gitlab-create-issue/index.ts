import type { SkillContext, SkillResult } from '../../types';

interface GitLabCreateIssueParams {
  projectId: string;
  title: string;
  description?: string;
  labels?: string[];
  assigneeIds?: number[];
}

export async function execute(
  context: SkillContext,
  params: GitLabCreateIssueParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.projectId || !params.title) {
    return {
      success: false,
      error: 'projectId and title are required',
    };
  }

  try {
    const response = await gateway.call('gitlab.createIssue', {
      projectId: params.projectId,
      title: params.title,
      description: params.description,
      labels: params.labels || [],
      assigneeIds: params.assigneeIds || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create GitLab issue',
    };
  }
}
