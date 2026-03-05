import type { SkillContext, SkillResult } from '../../types';

interface NodesListParams {
  status?: 'online' | 'offline' | 'all';
  includeCapabilities?: boolean;
}

export async function execute(
  context: SkillContext,
  params: NodesListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('nodes.list', {
      status: params.status || 'online',
      includeCapabilities: params.includeCapabilities !== false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list nodes',
    };
  }
}
