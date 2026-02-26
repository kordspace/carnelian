import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AmazonSearchProductsParams {
  keywords: string;
  category?: string;
  minPrice?: number;
  maxPrice?: number;
  sortBy?: string;
}

export async function execute(
  context: SkillContext,
  params: AmazonSearchProductsParams
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
    const response = await gateway.call('amazon.searchProducts', {
      keywords: params.keywords,
      category: params.category,
      minPrice: params.minPrice,
      maxPrice: params.maxPrice,
      sortBy: params.sortBy,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to search Amazon products',
    };
  }
}
