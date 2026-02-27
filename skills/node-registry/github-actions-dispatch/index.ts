import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GitHubActionsDispatchParams {
  owner: string;
  repo: string;
  workflowId: string;
  ref?: string;
  inputs?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: GitHubActionsDispatchParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.owner || !params.repo || !params.workflowId) {
    return {
      success: false,
      error: 'owner, repo, and workflowId are required',
    };
  }

  try {
    const response = await gateway.call('github.actionsDispatch', {
      owner: params.owner,
      repo: params.repo,
      workflowId: params.workflowId,
      ref: params.ref || 'main',
      inputs: params.inputs || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to dispatch GitHub Actions workflow',
    };
  }
}
