import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TransformMapParams {
  items: unknown[];
  skill: string;
  params?: Record<string, unknown>;
}

export async function execute(
  context: SkillContext,
  params: TransformMapParams
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
    const response = await gateway.call('transform.map', {
      items: params.items,
      skill: params.skill,
      params: params.params || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to map transform',
    };
  }
}
