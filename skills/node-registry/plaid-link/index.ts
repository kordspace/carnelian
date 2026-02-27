import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PlaidLinkParams {
  userId: string;
  products: string[];
  countryCodes?: string[];
  language?: string;
  webhookUrl?: string;
}

export async function execute(
  context: SkillContext,
  params: PlaidLinkParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.userId || !params.products || params.products.length === 0) {
    return {
      success: false,
      error: 'userId and products are required',
    };
  }

  try {
    const response = await gateway.call('plaid.link', {
      userId: params.userId,
      products: params.products,
      countryCodes: params.countryCodes || ['US'],
      language: params.language || 'en',
      webhookUrl: params.webhookUrl,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Plaid link token',
    };
  }
}
