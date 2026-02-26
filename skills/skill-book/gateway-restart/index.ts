import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GatewayRestartParams {
  graceful?: boolean;
}

export async function execute(
  context: SkillContext,
  params: GatewayRestartParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('gateway.restart', {
      graceful: params.graceful ?? true,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to restart gateway',
    };
  }
}
