import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AppleContactsCreateParams {
  firstName: string;
  lastName?: string;
  email?: string;
  phone?: string;
  company?: string;
  notes?: string;
}

export async function execute(
  context: SkillContext,
  params: AppleContactsCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.firstName) {
    return {
      success: false,
      error: 'firstName is required',
    };
  }

  try {
    const response = await gateway.call('apple.contacts.create', {
      firstName: params.firstName,
      lastName: params.lastName,
      email: params.email,
      phone: params.phone,
      company: params.company,
      notes: params.notes,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Apple contact',
    };
  }
}
