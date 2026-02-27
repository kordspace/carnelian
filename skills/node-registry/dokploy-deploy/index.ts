import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface DokployDeployParams {
  projectId: string;
  action?: 'deploy' | 'redeploy' | 'stop' | 'start' | 'status';
  branch?: string;
}

export async function execute(
  context: SkillContext,
  params: DokployDeployParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.projectId) {
    return {
      success: false,
      error: 'projectId is required',
    };
  }

  try {
    const response = await gateway.call('dokploy.deploy', {
      projectId: params.projectId,
      action: params.action || 'deploy',
      branch: params.branch,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Dokploy action',
    };
  }
}
