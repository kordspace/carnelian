import type { SkillContext, SkillResult } from '../../types';

interface PayPalCreatePaymentParams {
  amount: string;
  currency: string;
  description?: string;
  returnUrl: string;
  cancelUrl: string;
}

export async function execute(
  context: SkillContext,
  params: PayPalCreatePaymentParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.amount || !params.currency || !params.returnUrl || !params.cancelUrl) {
    return {
      success: false,
      error: 'amount, currency, returnUrl, and cancelUrl are required',
    };
  }

  try {
    const response = await gateway.call('paypal.createPayment', {
      amount: params.amount,
      currency: params.currency,
      description: params.description || '',
      returnUrl: params.returnUrl,
      cancelUrl: params.cancelUrl,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create PayPal payment',
    };
  }
}
