import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface DigitalOceanDeployParams {
  appId: string;
  forceRebuild?: boolean;
}

export async function execute(
  context: SkillContext,
  params: DigitalOceanDeployParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.appId) {
    return {
      success: false,
      error: 'appId is required',
    };
  }

  try {
    const response = await gateway.call('digitalocean.deploy', {
      appId: params.appId,
      forceRebuild: params.forceRebuild || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to deploy to DigitalOcean',
    };
  }
}
