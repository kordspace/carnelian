import type { SkillContext, SkillResult } from '../../types';

interface HealthKitSyncParams {
  dataType: 'steps' | 'heart-rate' | 'sleep' | 'workouts' | 'all';
  startDate?: string;
  endDate?: string;
}

export async function execute(
  context: SkillContext,
  params: HealthKitSyncParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.dataType) {
    return {
      success: false,
      error: 'dataType is required',
    };
  }

  try {
    const response = await gateway.call('healthkit.sync', {
      dataType: params.dataType,
      startDate: params.startDate,
      endDate: params.endDate,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to sync HealthKit data',
    };
  }
}
