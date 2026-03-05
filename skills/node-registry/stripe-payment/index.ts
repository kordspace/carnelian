import type { SkillContext, SkillResult } from '../../types';

interface StripePaymentParams {
  action: 'create_payment' | 'create_customer' | 'list_payments' | 'refund';
  amount?: number;
  currency?: string;
  customerId?: string;
  paymentIntentId?: string;
  email?: string;
  description?: string;
}

export async function execute(
  context: SkillContext,
  params: StripePaymentParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('stripe.payment', {
      action: params.action,
      amount: params.amount,
      currency: params.currency || 'usd',
      customerId: params.customerId,
      paymentIntentId: params.paymentIntentId,
      email: params.email,
      description: params.description,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Stripe payment action',
    };
  }
}
