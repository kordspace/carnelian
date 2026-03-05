import type { SkillContext, SkillResult } from '../../types';

interface PlaidLinkTokenParams {
  userId: string;
  clientName: string;
  products?: string[];
}

export async function execute(
  context: SkillContext,
  params: PlaidLinkTokenParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.userId || !params.clientName) {
    return {
      success: false,
      error: 'userId and clientName are required',
    };
  }

  try {
    const response = await gateway.call('plaid.createLinkToken', {
      userId: params.userId,
      clientName: params.clientName,
      products: params.products || ['transactions'],
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
