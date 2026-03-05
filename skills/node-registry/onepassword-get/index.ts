import type { SkillContext, SkillResult } from '../../types';

interface OnePasswordGetParams {
  itemName?: string;
  vault?: string;
  field?: string;
}

export async function execute(
  context: SkillContext,
  params: OnePasswordGetParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('onepassword.get', {
      itemName: params.itemName,
      vault: params.vault,
      field: params.field,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get 1Password item',
    };
  }
}
