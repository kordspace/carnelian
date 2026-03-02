import type { SkillContext, SkillResult } from '../../types';

interface GitHubCreatePRParams {
  owner: string;
  repo: string;
  title: string;
  head: string;
  base: string;
  body?: string;
  draft?: boolean;
}

export async function execute(
  context: SkillContext,
  params: GitHubCreatePRParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.owner || !params.repo || !params.title || !params.head || !params.base) {
    return {
      success: false,
      error: 'owner, repo, title, head, and base are required',
    };
  }

  try {
    const response = await gateway.call('github.createPR', {
      owner: params.owner,
      repo: params.repo,
      title: params.title,
      head: params.head,
      base: params.base,
      body: params.body || '',
      draft: params.draft || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create GitHub PR',
    };
  }
}
