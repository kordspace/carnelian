import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface JenkinsBuildParams {
  jobName: string;
  parameters?: Record<string, any>;
  token?: string;
}

export async function execute(
  context: SkillContext,
  params: JenkinsBuildParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.jobName) {
    return {
      success: false,
      error: 'jobName is required',
    };
  }

  try {
    const response = await gateway.call('jenkins.build', {
      jobName: params.jobName,
      parameters: params.parameters || {},
      token: params.token,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to trigger Jenkins build',
    };
  }
}
