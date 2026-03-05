import type { SkillContext, SkillResult } from '../../types';

interface SystemInfoParams {
  includeMemory?: boolean;
  includeCpu?: boolean;
  includeDisk?: boolean;
  includeNetwork?: boolean;
}

export async function execute(
  context: SkillContext,
  params: SystemInfoParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('system.info', {
      includeMemory: params.includeMemory !== false,
      includeCpu: params.includeCpu !== false,
      includeDisk: params.includeDisk !== false,
      includeNetwork: params.includeNetwork !== false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get system info',
    };
  }
}
