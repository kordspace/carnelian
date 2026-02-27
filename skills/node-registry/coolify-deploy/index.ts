import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CoolifyDeployParams {
  applicationId: string;
  action?: 'deploy' | 'restart' | 'stop' | 'status';
  force?: boolean;
}

export async function execute(
  context: SkillContext,
  params: CoolifyDeployParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.applicationId) {
    return {
      success: false,
      error: 'applicationId is required',
    };
  }

  try {
    const response = await gateway.call('coolify.deploy', {
      applicationId: params.applicationId,
      action: params.action || 'deploy',
      force: params.force || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Coolify action',
    };
  }
}
