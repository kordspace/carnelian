import type { SkillContext, SkillResult } from '../../types';

interface WorkflowStatusParams {
  workflowId: string;
}

export async function execute(
  context: SkillContext,
  params: WorkflowStatusParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.workflowId) {
    return {
      success: false,
      error: 'workflowId is required',
    };
  }

  try {
    const response = await gateway.call('workflow.status', {
      workflowId: params.workflowId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get workflow status',
    };
  }
}
