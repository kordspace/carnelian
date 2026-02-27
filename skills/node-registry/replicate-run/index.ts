import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ReplicateRunParams {
  model: string;
  version?: string;
  input: Record<string, any>;
  webhook?: string;
}

export async function execute(
  context: SkillContext,
  params: ReplicateRunParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.model || !params.input) {
    return {
      success: false,
      error: 'model and input are required',
    };
  }

  try {
    const response = await gateway.call('replicate.run', {
      model: params.model,
      version: params.version,
      input: params.input,
      webhook: params.webhook,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to run Replicate model',
    };
  }
}
