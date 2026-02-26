import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WooCommerceCreateProductParams {
  name: string;
  type?: 'simple' | 'grouped' | 'external' | 'variable';
  regularPrice?: string;
  description?: string;
  shortDescription?: string;
  categories?: number[];
  images?: Array<{ src: string }>;
}

export async function execute(
  context: SkillContext,
  params: WooCommerceCreateProductParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name) {
    return {
      success: false,
      error: 'name is required',
    };
  }

  try {
    const response = await gateway.call('woocommerce.createProduct', {
      name: params.name,
      type: params.type || 'simple',
      regularPrice: params.regularPrice,
      description: params.description,
      shortDescription: params.shortDescription,
      categories: params.categories || [],
      images: params.images || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create WooCommerce product',
    };
  }
}
