import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ShopifyCreateProductParams {
  title: string;
  description?: string;
  price: string;
  sku?: string;
  inventory?: number;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: ShopifyCreateProductParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.price) {
    return {
      success: false,
      error: 'title and price are required',
    };
  }

  try {
    const response = await gateway.call('shopify.createProduct', {
      title: params.title,
      description: params.description || '',
      price: params.price,
      sku: params.sku,
      inventory: params.inventory || 0,
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Shopify product',
    };
  }
}
