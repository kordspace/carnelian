import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PerformanceMeasureParams {
  name: string;
  startMark?: string;
  endMark?: string;
}

export async function execute(
  context: SkillContext,
  params: PerformanceMeasureParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name) {
    return {
      success: false,
      error: 'name is required',
    };
  }

  try {
    const response = await gateway.call('performance.measure', {
      name: params.name,
      startMark: params.startMark,
      endMark: params.endMark,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to measure performance',
    };
  }
}
