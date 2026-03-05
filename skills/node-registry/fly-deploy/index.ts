import type { SkillContext, SkillResult } from '../../types';

interface FlyDeployParams {
  appName: string;
  region?: string;
  config?: string;
  strategy?: string;
}

export async function execute(
  context: SkillContext,
  params: FlyDeployParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.appName) {
    return {
      success: false,
      error: 'appName is required',
    };
  }

  try {
    const response = await gateway.call('fly.deploy', {
      appName: params.appName,
      region: params.region,
      config: params.config,
      strategy: params.strategy || 'rolling',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to deploy to Fly.io',
    };
  }
}
