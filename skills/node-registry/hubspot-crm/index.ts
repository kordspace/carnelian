import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface HubSpotCRMParams {
  action: 'create_contact' | 'update_contact' | 'create_deal' | 'get_company' | 'search';
  objectType?: string;
  objectId?: string;
  properties?: Record<string, unknown>;
  searchQuery?: string;
}

export async function execute(
  context: SkillContext,
  params: HubSpotCRMParams
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
    const response = await gateway.call('hubspot.crm', {
      action: params.action,
      objectType: params.objectType,
      objectId: params.objectId,
      properties: params.properties,
      searchQuery: params.searchQuery,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute HubSpot CRM action',
    };
  }
}
