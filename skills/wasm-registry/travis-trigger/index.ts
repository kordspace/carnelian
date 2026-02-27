import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TravisTriggerParams {
  repository: string;
  branch?: string;
  message?: string;
}

export async function execute(
  context: SkillContext,
  params: TravisTriggerParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.repository) {
    return {
      success: false,
      error: 'repository is required',
    };
  }

  try {
    const response = await gateway.call('travis.trigger', {
      repository: params.repository,
      branch: params.branch || 'master',
      message: params.message,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to trigger Travis CI build',
    };
  }
}
