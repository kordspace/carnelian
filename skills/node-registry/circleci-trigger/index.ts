import type { SkillContext, SkillResult } from '../../types';

interface CircleCITriggerParams {
  projectSlug: string;
  branch?: string;
  parameters?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: CircleCITriggerParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.projectSlug) {
    return {
      success: false,
      error: 'projectSlug is required',
    };
  }

  try {
    const response = await gateway.call('circleci.trigger', {
      projectSlug: params.projectSlug,
      branch: params.branch || 'main',
      parameters: params.parameters || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to trigger CircleCI pipeline',
    };
  }
}
