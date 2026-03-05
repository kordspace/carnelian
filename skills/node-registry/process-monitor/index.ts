import type { SkillContext, SkillResult } from '../../types';

interface ProcessMonitorParams {
  pid?: number;
  name?: string;
  includeChildren?: boolean;
}

export async function execute(
  context: SkillContext,
  params: ProcessMonitorParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.pid && !params.name) {
    return {
      success: false,
      error: 'Either pid or name is required',
    };
  }

  try {
    const response = await gateway.call('process.monitor', {
      pid: params.pid,
      name: params.name,
      includeChildren: params.includeChildren || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to monitor process',
    };
  }
}
