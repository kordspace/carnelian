import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface StreamPipeParams {
  source: string;
  destination: string;
  transform?: string;
}

export async function execute(
  context: SkillContext,
  params: StreamPipeParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.source || !params.destination) {
    return {
      success: false,
      error: 'source and destination are required',
    };
  }

  try {
    const response = await gateway.call('stream.pipe', {
      source: params.source,
      destination: params.destination,
      transform: params.transform,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to pipe stream',
    };
  }
}
