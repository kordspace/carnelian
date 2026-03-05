import type { SkillContext, SkillResult } from '../../types';

interface MetricsTrackParams {
  metric: string;
  value: number;
  unit?: string;
  tags?: Record<string, string>;
  timestamp?: string;
}

export async function execute(
  context: SkillContext,
  params: MetricsTrackParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.metric || params.value === undefined) {
    return {
      success: false,
      error: 'metric and value are required',
    };
  }

  try {
    const response = await gateway.call('metrics.track', {
      metric: params.metric,
      value: params.value,
      unit: params.unit || 'count',
      tags: params.tags || {},
      timestamp: params.timestamp || new Date().toISOString(),
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
