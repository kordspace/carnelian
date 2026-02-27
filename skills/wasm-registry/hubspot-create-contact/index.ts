import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface HubSpotCreateContactParams {
  email: string;
  firstName?: string;
  lastName?: string;
  phone?: string;
  company?: string;
}

export async function execute(
  context: SkillContext,
  params: HubSpotCreateContactParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.email) {
    return {
      success: false,
      error: 'email is required',
    };
  }

  try {
    const response = await gateway.call('hubspot.createContact', {
      email: params.email,
      firstName: params.firstName,
      lastName: params.lastName,
      phone: params.phone,
      company: params.company,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create HubSpot contact',
    };
  }
}
