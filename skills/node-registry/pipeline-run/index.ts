import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PipelineRunParams {
  steps: Array<{
    skill: string;
    params: Record<string, unknown>;
  }>;
  stopOnError?: boolean;
}

export async function execute(
  context: SkillContext,
  params: PipelineRunParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.steps || params.steps.length === 0) {
    return {
      success: false,
      error: 'steps array is required and must not be empty',
    };
  }

  try {
    const response = await gateway.call('pipeline.run', {
      steps: params.steps,
      stopOnError: params.stopOnError !== false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to run pipeline',
    };
  }
}
