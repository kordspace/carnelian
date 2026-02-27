import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CoordinatorElectParams {
  group: string;
  nodeId: string;
  ttl?: number;
}

export async function execute(
  context: SkillContext,
  params: CoordinatorElectParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.group || !params.nodeId) {
    return {
      success: false,
      error: 'group and nodeId are required',
    };
  }

  try {
    const response = await gateway.call('coordinator.elect', {
      group: params.group,
      nodeId: params.nodeId,
      ttl: params.ttl || 30000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to elect coordinator',
    };
  }
}
