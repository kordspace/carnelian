import type { SkillContext, SkillResult } from '../../types';

interface SemaphoreAcquireParams {
  name: string;
  permits?: number;
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: SemaphoreAcquireParams
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
    const response = await gateway.call('semaphore.acquire', {
      name: params.name,
      permits: params.permits || 1,
      timeout: params.timeout || 5000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to acquire semaphore',
    };
  }
}
