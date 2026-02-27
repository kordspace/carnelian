import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PrometheusQueryParams {
  query: string;
  time?: number;
}

export async function execute(
  context: SkillContext,
  params: PrometheusQueryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.query) {
    return {
      success: false,
      error: 'query is required',
    };
  }

  try {
    const response = await gateway.call('prometheus.query', {
      query: params.query,
      time: params.time || Date.now() / 1000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to query Prometheus',
    };
  }
}
