import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CronListParams {
  includeDisabled?: boolean;
}

export async function execute(
  context: SkillContext,
  params: CronListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('cron.list', {
      includeDisabled: params.includeDisabled ?? false,
    });

    const jobs = Array.isArray(response?.jobs) ? response.jobs : [];

    return {
      success: true,
      data: {
        count: jobs.length,
        jobs,
      },
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list cron jobs',
    };
  }
}
