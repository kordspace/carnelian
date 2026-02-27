import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface DatadogMetricParams {
  metric: string;
  value: number;
  type?: 'gauge' | 'count' | 'rate';
  tags?: string[];
  timestamp?: number;
}

export async function execute(
  context: SkillContext,
  params: DatadogMetricParams
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
    const response = await gateway.call('datadog.metric', {
      metric: params.metric,
      value: params.value,
      type: params.type || 'gauge',
      tags: params.tags || [],
      timestamp: params.timestamp || Math.floor(Date.now() / 1000),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Datadog metric',
    };
  }
}
