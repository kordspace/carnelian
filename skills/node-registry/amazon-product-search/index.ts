import type { SkillContext, SkillResult } from '../../types';

interface AmazonProductSearchParams {
  keywords: string;
  category?: string;
  minPrice?: number;
  maxPrice?: number;
  page?: number;
}

export async function execute(
  context: SkillContext,
  params: AmazonProductSearchParams
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
    const response = await gateway.call('amazon.search', {
      keywords: params.keywords,
      category: params.category,
      minPrice: params.minPrice,
      maxPrice: params.maxPrice,
      page: params.page || 1,
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
