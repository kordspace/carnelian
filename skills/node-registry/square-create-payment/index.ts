import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SquareCreatePaymentParams {
  amount: number;
  currency: string;
  sourceId: string;
  note?: string;
}

export async function execute(
  context: SkillContext,
  params: SquareCreatePaymentParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.amount || !params.currency || !params.sourceId) {
    return {
      success: false,
      error: 'amount, currency, and sourceId are required',
    };
  }

  try {
    const response = await gateway.call('square.createPayment', {
      amount: params.amount,
      currency: params.currency,
      sourceId: params.sourceId,
      note: params.note,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Square payment',
    };
  }
}
