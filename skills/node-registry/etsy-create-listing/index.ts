import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface EtsyCreateListingParams {
  title: string;
  description: string;
  price: number;
  quantity: number;
  shopId: number;
  whoMade?: 'i_did' | 'collective' | 'someone_else';
  whenMade?: string;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: EtsyCreateListingParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.description || !params.price || !params.quantity || !params.shopId) {
    return {
      success: false,
      error: 'title, description, price, quantity, and shopId are required',
    };
  }

  try {
    const response = await gateway.call('etsy.createListing', {
      title: params.title,
      description: params.description,
      price: params.price,
      quantity: params.quantity,
      shopId: params.shopId,
      whoMade: params.whoMade || 'i_did',
      whenMade: params.whenMade,
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Etsy listing',
    };
  }
}
