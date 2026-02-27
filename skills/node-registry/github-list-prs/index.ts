import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GitHubListPRsParams {
  owner: string;
  repo: string;
  state?: 'open' | 'closed' | 'all';
  sort?: 'created' | 'updated' | 'popularity';
  direction?: 'asc' | 'desc';
}

export async function execute(
  context: SkillContext,
  params: GitHubListPRsParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.owner || !params.repo) {
    return {
      success: false,
      error: 'owner and repo are required',
    };
  }

  try {
    const response = await gateway.call('github.listPRs', {
      owner: params.owner,
      repo: params.repo,
      state: params.state || 'open',
      sort: params.sort || 'created',
      direction: params.direction || 'desc',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list GitHub PRs',
    };
  }
}
