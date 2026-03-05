import type { SkillContext, SkillResult } from '../../types';

interface EbaySearchParams {
  keywords: string;
  categoryId?: string;
  condition?: string;
  sortOrder?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: EbaySearchParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.keywords) {
    return {
      success: false,
      error: 'keywords is required',
    };
  }

  try {
    const response = await gateway.call('ebay.search', {
      keywords: params.keywords,
      categoryId: params.categoryId,
      condition: params.condition,
      sortOrder: params.sortOrder || 'BestMatch',
      limit: params.limit || 50,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to search eBay',
    };
  }
}
