import type { SkillContext, SkillResult } from '../../types';

interface FastlyPurgeParams {
  serviceId: string;
  key?: string;
  url?: string;
  surrogate?: boolean;
}

export async function execute(
  context: SkillContext,
  params: FastlyPurgeParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.serviceId) {
    return {
      success: false,
      error: 'serviceId is required',
    };
  }

  try {
    const response = await gateway.call('fastly.purge', {
      serviceId: params.serviceId,
      key: params.key,
      url: params.url,
      surrogate: params.surrogate || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to purge Fastly cache',
    };
  }
}
