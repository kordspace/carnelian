import type { SkillContext, SkillResult } from '../../types';

interface EbayListItemParams {
  title: string;
  description: string;
  categoryId: string;
  startPrice: number;
  quantity?: number;
  duration?: number;
  images?: string[];
}

export async function execute(
  context: SkillContext,
  params: EbayListItemParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.description || !params.categoryId || !params.startPrice) {
    return {
      success: false,
      error: 'title, description, categoryId, and startPrice are required',
    };
  }

  try {
    const response = await gateway.call('ebay.listItem', {
      title: params.title,
      description: params.description,
      categoryId: params.categoryId,
      startPrice: params.startPrice,
      quantity: params.quantity || 1,
      duration: params.duration || 7,
      images: params.images || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list item on eBay',
    };
  }
}
