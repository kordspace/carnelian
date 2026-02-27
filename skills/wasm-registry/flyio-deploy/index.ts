import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface FlyIODeployParams {
  appName: string;
  action?: 'deploy' | 'scale' | 'restart' | 'status';
  region?: string;
  vmSize?: string;
  count?: number;
}

export async function execute(
  context: SkillContext,
  params: FlyIODeployParams
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
    const response = await gateway.call('flyio.deploy', {
      appName: params.appName,
      action: params.action || 'deploy',
      region: params.region,
      vmSize: params.vmSize,
      count: params.count,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Fly.io action',
    };
  }
}
