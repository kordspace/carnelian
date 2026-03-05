import type { SkillContext, SkillResult } from '../../types';

interface PayPalPaymentParams {
  action: 'create_order' | 'capture_order' | 'refund' | 'get_order';
  amount?: number;
  currency?: string;
  orderId?: string;
  description?: string;
  returnUrl?: string;
  cancelUrl?: string;
}

export async function execute(
  context: SkillContext,
  params: PayPalPaymentParams
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
    const response = await gateway.call('paypal.payment', {
      action: params.action,
      amount: params.amount,
      currency: params.currency || 'USD',
      orderId: params.orderId,
      description: params.description,
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
      error: error instanceof Error ? error.message : 'Failed to execute PayPal payment action',
    };
  }
}
