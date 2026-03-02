import type { SkillContext, SkillResult } from '../../types';

interface BitwardenGetParams {
  itemId?: string;
  search?: string;
  type?: 'login' | 'note' | 'card' | 'identity';
}

export async function execute(
  context: SkillContext,
  params: BitwardenGetParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('bitwarden.get', {
      itemId: params.itemId,
      search: params.search,
      type: params.type,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get Bitwarden item',
    };
  }
}
