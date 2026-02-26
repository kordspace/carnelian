import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface VercelDeployParams {
  projectId: string;
  gitBranch?: string;
  environment?: 'production' | 'preview' | 'development';
  buildCommand?: string;
}

export async function execute(
  context: SkillContext,
  params: VercelDeployParams
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
    const response = await gateway.call('vercel.deploy', {
      projectId: params.projectId,
      gitBranch: params.gitBranch,
      environment: params.environment || 'preview',
      buildCommand: params.buildCommand,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to deploy to Vercel',
    };
  }
}
