import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AuthVerifyParams {
  token: string;
  type?: 'jwt' | 'bearer' | 'api-key';
  secret?: string;
}

export async function execute(
  context: SkillContext,
  params: AuthVerifyParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.token) {
    return {
      success: false,
      error: 'token is required',
    };
  }

  try {
    const response = await gateway.call('auth.verify', {
      token: params.token,
      type: params.type || 'jwt',
      secret: params.secret,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to verify authentication',
    };
  }
}
