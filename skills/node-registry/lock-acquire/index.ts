import type { SkillContext, SkillResult } from '../../types';

interface LockAcquireParams {
  resource: string;
  timeout?: number;
  ttl?: number;
}

export async function execute(
  context: SkillContext,
  params: LockAcquireParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.resource) {
    return {
      success: false,
      error: 'resource is required',
    };
  }

  try {
    const response = await gateway.call('lock.acquire', {
      resource: params.resource,
      timeout: params.timeout || 5000,
      ttl: params.ttl || 30000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to acquire lock',
    };
  }
}
