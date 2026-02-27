import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GarminActivityParams {
  startDate?: string;
  endDate?: string;
  activityType?: string;
}

export async function execute(
  context: SkillContext,
  params: GarminActivityParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('garmin.activity', {
      startDate: params.startDate,
      endDate: params.endDate,
      activityType: params.activityType,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch Garmin activity',
    };
  }
}
