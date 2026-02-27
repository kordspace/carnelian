import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GitHubCreateIssueParams {
  owner: string;
  repo: string;
  title: string;
  body?: string;
  labels?: string[];
  assignees?: string[];
}

export async function execute(
  context: SkillContext,
  params: GitHubCreateIssueParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.owner || !params.repo || !params.title) {
    return {
      success: false,
      error: 'owner, repo, and title are required',
    };
  }

  try {
    const response = await gateway.call('github.createIssue', {
      owner: params.owner,
      repo: params.repo,
      title: params.title,
      body: params.body || '',
      labels: params.labels || [],
      assignees: params.assignees || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create GitHub issue',
    };
  }
}
