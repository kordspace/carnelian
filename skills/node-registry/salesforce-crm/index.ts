import type { SkillContext, SkillResult } from '../../types';

interface SalesforceCRMParams {
  action: 'create_lead' | 'update_lead' | 'query' | 'create_opportunity' | 'get_account';
  objectType?: string;
  recordId?: string;
  data?: Record<string, unknown>;
  query?: string;
}

export async function execute(
  context: SkillContext,
  params: SalesforceCRMParams
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
    const response = await gateway.call('salesforce.crm', {
      action: params.action,
      objectType: params.objectType,
      recordId: params.recordId,
      data: params.data,
      query: params.query,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Salesforce CRM action',
    };
  }
}
