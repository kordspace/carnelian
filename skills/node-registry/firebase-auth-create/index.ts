import type { SkillContext, SkillResult } from '../../types';

interface FirebaseAuthCreateParams {
  email: string;
  password: string;
  displayName?: string;
}

export async function execute(
  context: SkillContext,
  params: FirebaseAuthCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.email || !params.password) {
    return {
      success: false,
      error: 'email and password are required',
    };
  }

  try {
    const response = await gateway.call('firebase.authCreate', {
      email: params.email,
      password: params.password,
      displayName: params.displayName,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Firebase user',
    };
  }
}
