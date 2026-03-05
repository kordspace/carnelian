import type { SkillContext, SkillResult } from '../../types';

interface RenderDeployParams {
  serviceId: string;
  clearCache?: boolean;
}

export async function execute(
  context: SkillContext,
  params: RenderDeployParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.serviceId) {
    return {
      success: false,
      error: 'serviceId is required',
    };
  }

  try {
    const response = await gateway.call('render.deploy', {
      serviceId: params.serviceId,
      clearCache: params.clearCache || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to deploy to Render',
    };
  }
}
