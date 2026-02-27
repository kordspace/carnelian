import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface DashlaneGetParams {
  search?: string;
  type?: 'password' | 'note' | 'payment';
  url?: string;
}

export async function execute(
  context: SkillContext,
  params: DashlaneGetParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('dashlane.get', {
      search: params.search,
      type: params.type || 'password',
      url: params.url,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get Dashlane item',
    };
  }
}
