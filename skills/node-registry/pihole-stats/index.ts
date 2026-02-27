import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PiHoleStatsParams {
  action?: 'summary' | 'top-queries' | 'top-ads' | 'recent-blocked';
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: PiHoleStatsParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('pihole.stats', {
      action: params.action || 'summary',
      limit: params.limit || 10,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get Pi-hole stats',
    };
  }
}
