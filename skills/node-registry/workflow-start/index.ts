import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WorkflowStartParams {
  name: string;
  input?: any;
  tags?: Record<string, string>;
}

export async function execute(
  context: SkillContext,
  params: WorkflowStartParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name) {
    return {
      success: false,
      error: 'name is required',
    };
  }

  try {
    const response = await gateway.call('workflow.start', {
      name: params.name,
      input: params.input,
      tags: params.tags || {},
      timestamp: Date.now(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to start workflow',
    };
  }
}
