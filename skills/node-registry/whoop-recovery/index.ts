import type { SkillContext, SkillResult } from '../../types';

interface WhoopRecoveryParams {
  startDate?: string;
  endDate?: string;
}

export async function execute(
  context: SkillContext,
  params: WhoopRecoveryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('whoop.recovery', {
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
      error: error instanceof Error ? error.message : 'Failed to fetch WHOOP recovery data',
    };
  }
}
