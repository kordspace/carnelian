import type { SkillContext, SkillResult } from '../../types';

interface HerokuDeployParams {
  appName: string;
  tarballUrl?: string;
  sourceBlob?: string;
}

export async function execute(
  context: SkillContext,
  params: HerokuDeployParams
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
    const response = await gateway.call('heroku.deploy', {
      appName: params.appName,
      tarballUrl: params.tarballUrl,
      sourceBlob: params.sourceBlob,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to deploy to Heroku',
    };
  }
}
