import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BitbucketCreatePRParams {
  workspace: string;
  repoSlug: string;
  title: string;
  sourceBranch: string;
  destinationBranch: string;
  description?: string;
  closeSourceBranch?: boolean;
}

export async function execute(
  context: SkillContext,
  params: BitbucketCreatePRParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.workspace || !params.repoSlug || !params.title || !params.sourceBranch || !params.destinationBranch) {
    return {
      success: false,
      error: 'workspace, repoSlug, title, sourceBranch, and destinationBranch are required',
    };
  }

  try {
    const response = await gateway.call('bitbucket.createPR', {
      workspace: params.workspace,
      repoSlug: params.repoSlug,
      title: params.title,
      sourceBranch: params.sourceBranch,
      destinationBranch: params.destinationBranch,
      description: params.description,
      closeSourceBranch: params.closeSourceBranch || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Bitbucket pull request',
    };
  }
}
