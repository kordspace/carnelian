import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BatchProcessParams {
  items: unknown[];
  skill: string;
  batchSize?: number;
  parallel?: boolean;
}

export async function execute(
  context: SkillContext,
  params: BatchProcessParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.items || !params.skill) {
    return {
      success: false,
      error: 'items and skill are required',
    };
  }

  try {
    const response = await gateway.call('batch.process', {
      items: params.items,
      skill: params.skill,
      batchSize: params.batchSize || 10,
      parallel: params.parallel || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to process batch',
    };
  }
}
