import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface YNABCreateTransactionParams {
  budgetId: string;
  accountId: string;
  date: string;
  amount: number;
  payeeName?: string;
  categoryId?: string;
  memo?: string;
}

export async function execute(
  context: SkillContext,
  params: YNABCreateTransactionParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.budgetId || !params.accountId || !params.date || !params.amount) {
    return {
      success: false,
      error: 'budgetId, accountId, date, and amount are required',
    };
  }

  try {
    const response = await gateway.call('ynab.createTransaction', {
      budgetId: params.budgetId,
      accountId: params.accountId,
      date: params.date,
      amount: params.amount,
      payeeName: params.payeeName,
      categoryId: params.categoryId,
      memo: params.memo,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create YNAB transaction',
    };
  }
}
