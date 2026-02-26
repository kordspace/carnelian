import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface HealthCheckParams {
  service?: string;
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: HealthCheckParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('health.check', {
      service: params.service || 'all',
      timeout: params.timeout || 5000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to check health',
    };
  }
}
