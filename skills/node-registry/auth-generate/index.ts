import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AuthGenerateParams {
  payload: Record<string, unknown>;
  type?: 'jwt' | 'bearer' | 'api-key';
  secret?: string;
  expiresIn?: number;
}

export async function execute(
  context: SkillContext,
  params: AuthGenerateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.payload) {
    return {
      success: false,
      error: 'payload is required',
    };
  }

  try {
    const response = await gateway.call('auth.generate', {
      payload: params.payload,
      type: params.type || 'jwt',
      secret: params.secret,
      expiresIn: params.expiresIn || 3600,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate authentication token',
    };
  }
}
