import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface LockReleaseParams {
  resource: string;
  lockId: string;
}

export async function execute(
  context: SkillContext,
  params: LockReleaseParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.resource || !params.lockId) {
    return {
      success: false,
      error: 'resource and lockId are required',
    };
  }

  try {
    const response = await gateway.call('lock.release', {
      resource: params.resource,
      lockId: params.lockId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to release lock',
    };
  }
}
