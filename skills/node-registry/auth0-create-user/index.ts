import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface Auth0CreateUserParams {
  email: string;
  password: string;
  name?: string;
  connection?: string;
}

export async function execute(
  context: SkillContext,
  params: Auth0CreateUserParams
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
    const response = await gateway.call('auth0.createUser', {
      email: params.email,
      password: params.password,
      name: params.name,
      connection: params.connection || 'Username-Password-Authentication',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Auth0 user',
    };
  }
}
