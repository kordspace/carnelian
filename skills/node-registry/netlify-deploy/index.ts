import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface NetlifyDeployParams {
  siteId: string;
  dir: string;
  branch?: string;
  message?: string;
}

export async function execute(
  context: SkillContext,
  params: NetlifyDeployParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.siteId || !params.dir) {
    return {
      success: false,
      error: 'siteId and dir are required',
    };
  }

  try {
    const response = await gateway.call('netlify.deploy', {
      siteId: params.siteId,
      dir: params.dir,
      branch: params.branch,
      message: params.message || 'Deploy from CARNELIAN',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to deploy to Netlify',
    };
  }
}
