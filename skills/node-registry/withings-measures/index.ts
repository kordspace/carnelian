import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WithingsMeasuresParams {
  startDate?: string;
  endDate?: string;
  measType?: number;
}

export async function execute(
  context: SkillContext,
  params: WithingsMeasuresParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('withings.measures', {
      startDate: params.startDate,
      endDate: params.endDate,
      measType: params.measType,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch Withings measures',
    };
  }
}
