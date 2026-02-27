import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TautulliStatsParams {
  statId?: string;
  timeRange?: number;
  userId?: number;
}

export async function execute(
  context: SkillContext,
  params: TautulliStatsParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('tautulli.stats', {
      statId: params.statId || 'plays_by_month',
      timeRange: params.timeRange || 30,
      userId: params.userId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch Tautulli stats',
    };
  }
}
