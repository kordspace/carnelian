import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AgentsListParams {
  status?: 'active' | 'inactive' | 'all';
  includeCapabilities?: boolean;
}

export async function execute(
  context: SkillContext,
  params: AgentsListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('agents.list', {
      status: params.status || 'active',
      includeCapabilities: params.includeCapabilities !== false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list agents',
    };
  }
}
