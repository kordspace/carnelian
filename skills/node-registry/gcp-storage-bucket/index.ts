import type { SkillContext, SkillResult } from '../../types';

interface GCPStorageBucketParams {
  action: 'create' | 'delete' | 'list' | 'upload' | 'download' | 'get';
  bucketName?: string;
  fileName?: string;
  localFilePath?: string;
  contentType?: string;
  prefix?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: GCPStorageBucketParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('gcp.storage', {
      action: params.action,
      bucketName: params.bucketName,
      fileName: params.fileName,
      localFilePath: params.localFilePath,
      contentType: params.contentType,
      prefix: params.prefix,
      limit: params.limit || 100,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute GCP Storage action' };
  }
}
