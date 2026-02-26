import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface StripeCreatePaymentParams {
  amount: number;
  currency: string;
  description?: string;
  customer?: string;
  metadata?: Record<string, string>;
}

export async function execute(
  context: SkillContext,
  params: StripeCreatePaymentParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.amount || !params.currency) {
    return {
      success: false,
      error: 'amount and currency are required',
    };
  }

  try {
    const response = await gateway.call('stripe.createPayment', {
      amount: params.amount,
      currency: params.currency,
      description: params.description || '',
      customer: params.customer,
      metadata: params.metadata || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Stripe payment',
    };
  }
}
