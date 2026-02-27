import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GitLabProjectParams {
  action: 'create_project' | 'create_issue' | 'create_merge_request' | 'list_projects' | 'get_project';
  name?: string;
  description?: string;
  projectId?: string;
  title?: string;
  sourceBranch?: string;
  targetBranch?: string;
  visibility?: 'private' | 'internal' | 'public';
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: GitLabProjectParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('gitlab.project', {
      action: params.action,
      name: params.name,
      description: params.description,
      projectId: params.projectId,
      title: params.title,
      sourceBranch: params.sourceBranch,
      targetBranch: params.targetBranch,
      visibility: params.visibility,
      limit: params.limit || 20,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute GitLab project action' };
  }
}
