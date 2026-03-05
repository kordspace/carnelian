import type { SkillContext, SkillResult } from '../../types';

interface StorageDeleteParams {
  key: string;
  bucket?: string;
}

export async function execute(
  context: SkillContext,
  params: StorageDeleteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key) {
    return {
      success: false,
      error: 'key is required',
    };
  }

  try {
    const response = await gateway.call('storage.delete', {
      key: params.key,
      bucket: params.bucket || 'default',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to delete from storage',
    };
  }
}
