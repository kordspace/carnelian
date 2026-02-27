import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SemaphoreReleaseParams {
  name: string;
  permits?: number;
}

export async function execute(
  context: SkillContext,
  params: SemaphoreReleaseParams
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
    const response = await gateway.call('semaphore.release', {
      name: params.name,
      permits: params.permits || 1,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to release semaphore',
    };
  }
}
