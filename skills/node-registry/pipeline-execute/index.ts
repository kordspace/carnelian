import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PipelineExecuteParams {
  stages: Array<{
    name: string;
    action: string;
    params?: Record<string, any>;
  }>;
  input?: any;
}

export async function execute(
  context: SkillContext,
  params: PipelineExecuteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.stages || params.stages.length === 0) {
    return {
      success: false,
      error: 'stages array is required and cannot be empty',
    };
  }

  try {
    const response = await gateway.call('pipeline.execute', {
      stages: params.stages,
      input: params.input,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute pipeline',
    };
  }
}
