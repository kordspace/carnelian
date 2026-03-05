import type { SkillContext, SkillResult } from '../../types';

interface SalesforceCreateLeadParams {
  lastName: string;
  company: string;
  email?: string;
  phone?: string;
  status?: string;
}

export async function execute(
  context: SkillContext,
  params: SalesforceCreateLeadParams
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
    const response = await gateway.call('salesforce.createLead', {
      lastName: params.lastName,
      company: params.company,
      email: params.email,
      phone: params.phone,
      status: params.status || 'Open - Not Contacted',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Salesforce lead',
    };
  }
}
