import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CascadeRunParams {
  workflow: string;
  input?: Record<string, unknown>;
  timeout?: number;
  async?: boolean;
}

export async function execute(
  context: SkillContext,
  params: CascadeRunParams
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
    const response = await gateway.call('cascade.run', {
      workflow: params.workflow,
      input: params.input || {},
      timeout: params.timeout || 60000,
      async: params.async || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to run cascade workflow',
    };
  }
}
