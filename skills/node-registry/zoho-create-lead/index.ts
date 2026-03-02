import type { SkillContext, SkillResult } from '../../types';

interface ZohoCreateLeadParams {
  lastName: string;
  company: string;
  email?: string;
  phone?: string;
  leadSource?: string;
  industry?: string;
}

export async function execute(
  context: SkillContext,
  params: ZohoCreateLeadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.lastName || !params.company) {
    return {
      success: false,
      error: 'lastName and company are required',
    };
  }

  try {
    const response = await gateway.call('zoho.createLead', {
      lastName: params.lastName,
      company: params.company,
      email: params.email,
      phone: params.phone,
      leadSource: params.leadSource,
      industry: params.industry,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Zoho lead',
    };
  }
}
