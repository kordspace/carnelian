import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface StorageWriteParams {
  key: string;
  value: unknown;
  bucket?: string;
  metadata?: Record<string, unknown>;
}

export async function execute(
  context: SkillContext,
  params: StorageWriteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key || params.value === undefined) {
    return {
      success: false,
      error: 'key and value are required',
    };
  }

  try {
    const response = await gateway.call('storage.write', {
      key: params.key,
      value: params.value,
      bucket: params.bucket || 'default',
      metadata: params.metadata || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to write to storage',
    };
  }
}
