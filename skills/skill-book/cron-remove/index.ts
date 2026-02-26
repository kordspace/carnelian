import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CronRemoveParams {
  jobId: string;
}

export async function execute(
  context: SkillContext,
  params: CronRemoveParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.jobId || typeof params.jobId !== 'string') {
    return {
      success: false,
      error: 'jobId is required',
    };
  }

  try {
    const response = await gateway.call('cron.remove', {
      id: params.jobId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to remove cron job',
    };
  }
}
