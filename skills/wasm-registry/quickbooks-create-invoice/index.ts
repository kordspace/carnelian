import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface QuickBooksCreateInvoiceParams {
  customerId: string;
  lineItems: Array<{
    description: string;
    amount: number;
    quantity?: number;
  }>;
  dueDate?: string;
  terms?: string;
}

export async function execute(
  context: SkillContext,
  params: QuickBooksCreateInvoiceParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.customerId || !params.lineItems || params.lineItems.length === 0) {
    return {
      success: false,
      error: 'customerId and lineItems are required',
    };
  }

  try {
    const response = await gateway.call('quickbooks.createInvoice', {
      customerId: params.customerId,
      lineItems: params.lineItems,
      dueDate: params.dueDate,
      terms: params.terms,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create QuickBooks invoice',
    };
  }
}
