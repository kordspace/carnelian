import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface MintTransactionsParams {
  startDate?: string;
  endDate?: string;
  accountId?: string;
  categoryId?: string;
}

export async function execute(
  context: SkillContext,
  params: MintTransactionsParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('mint.transactions', {
      startDate: params.startDate,
      endDate: params.endDate,
      accountId: params.accountId,
      categoryId: params.categoryId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch Mint transactions',
    };
  }
}
