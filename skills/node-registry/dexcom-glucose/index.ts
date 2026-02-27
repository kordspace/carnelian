import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface DexcomGlucoseParams {
  startDate?: string;
  endDate?: string;
  maxCount?: number;
}

export async function execute(
  context: SkillContext,
  params: DexcomGlucoseParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('dexcom.glucose', {
      startDate: params.startDate,
      endDate: params.endDate,
      maxCount: params.maxCount || 288,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get Dexcom glucose data',
    };
  }
}
