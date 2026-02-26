import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WorkflowExecuteParams {
  workflow: string;
  params?: Record<string, unknown>;
  async?: boolean;
}

export async function execute(
  context: SkillContext,
  params: WorkflowExecuteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.workflow) {
    return {
      success: false,
      error: 'workflow is required',
    };
  }

  try {
    const response = await gateway.call('workflow.execute', {
      workflow: params.workflow,
      params: params.params || {},
      async: params.async || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute workflow',
    };
  }
}
