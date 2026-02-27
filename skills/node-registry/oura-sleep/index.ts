import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface OuraSleepParams {
  startDate?: string;
  endDate?: string;
}

export async function execute(
  context: SkillContext,
  params: OuraSleepParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('oura.sleep', {
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
      error: error instanceof Error ? error.message : 'Failed to fetch Oura sleep data',
    };
  }
}
