import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface RailwayDeployParams {
  projectId: string;
  environment?: string;
  serviceId?: string;
}

export async function execute(
  context: SkillContext,
  params: RailwayDeployParams
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
    const response = await gateway.call('railway.deploy', {
      projectId: params.projectId,
      environment: params.environment || 'production',
      serviceId: params.serviceId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to deploy to Railway',
    };
  }
}
