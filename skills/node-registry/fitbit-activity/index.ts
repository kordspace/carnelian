import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface FitbitActivityParams {
  date?: string;
  period?: string;
}

export async function execute(
  context: SkillContext,
  params: FitbitActivityParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('fitbit.activity', {
      date: params.date || 'today',
      period: params.period || '1d',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch Fitbit activity',
    };
  }
}
