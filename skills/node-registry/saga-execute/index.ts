import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SagaExecuteParams {
  steps: Array<{
    action: string;
    params?: Record<string, any>;
    compensate?: string;
  }>;
}

export async function execute(
  context: SkillContext,
  params: SagaExecuteParams
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
      error: 'steps array is required and cannot be empty',
    };
  }

  try {
    const response = await gateway.call('saga.execute', {
      steps: params.steps,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute saga',
    };
  }
}
