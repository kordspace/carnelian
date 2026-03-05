import type { SkillContext, SkillResult } from '../../types';

interface MetricTrackParams {
  name: string;
  value: number;
  unit?: string;
  tags?: Record<string, string>;
}

export async function execute(
  context: SkillContext,
  params: MetricTrackParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name || params.value === undefined) {
    return {
      success: false,
      error: 'name and value are required',
    };
  }

  try {
    const response = await gateway.call('metric.track', {
      name: params.name,
      value: params.value,
      unit: params.unit || 'count',
      tags: params.tags || {},
      timestamp: Date.now(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to track metric',
    };
  }
}
