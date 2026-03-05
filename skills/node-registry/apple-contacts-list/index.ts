import type { SkillContext, SkillResult } from '../../types';

interface AppleContactsListParams {
  searchTerm?: string;
  limit?: number;
  fields?: string[];
}

export async function execute(
  context: SkillContext,
  params: AppleContactsListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('apple.contacts.list', {
      searchTerm: params.searchTerm,
      limit: params.limit || 100,
      fields: params.fields || ['name', 'email', 'phone'],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list Apple contacts',
    };
  }
}
