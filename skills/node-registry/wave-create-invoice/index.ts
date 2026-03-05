import type { SkillContext, SkillResult } from '../../types';

interface WaveCreateInvoiceParams {
  businessId: string;
  customerId: string;
  items: Array<{
    productId: string;
    quantity: number;
    unitPrice: number;
  }>;
  dueDate?: string;
}

export async function execute(
  context: SkillContext,
  params: WaveCreateInvoiceParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.businessId || !params.customerId || !params.items || params.items.length === 0) {
    return {
      success: false,
      error: 'businessId, customerId, and items are required',
    };
  }

  try {
    const response = await gateway.call('wave.createInvoice', {
      businessId: params.businessId,
      customerId: params.customerId,
      items: params.items,
      dueDate: params.dueDate,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Wave invoice',
    };
  }
}
