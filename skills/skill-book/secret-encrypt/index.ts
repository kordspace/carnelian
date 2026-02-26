import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SecretEncryptParams {
  data: string;
  key?: string;
  algorithm?: 'aes-256-gcm' | 'aes-128-gcm' | 'chacha20-poly1305';
}

export async function execute(
  context: SkillContext,
  params: SecretEncryptParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.data) {
    return {
      success: false,
      error: 'data is required',
    };
  }

  try {
    const response = await gateway.call('secret.encrypt', {
      data: params.data,
      key: params.key,
      algorithm: params.algorithm || 'aes-256-gcm',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to encrypt secret',
    };
  }
}
