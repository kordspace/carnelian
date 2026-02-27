import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GCPStorageUploadParams {
  bucket: string;
  filename: string;
  content: string | Buffer;
  contentType?: string;
  metadata?: Record<string, string>;
}

export async function execute(
  context: SkillContext,
  params: GCPStorageUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.bucket || !params.filename || !params.content) {
    return {
      success: false,
      error: 'bucket, filename, and content are required',
    };
  }

  try {
    const response = await gateway.call('gcp.storage.upload', {
      bucket: params.bucket,
      filename: params.filename,
      content: params.content,
      contentType: params.contentType,
      metadata: params.metadata,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to GCP Storage',
    };
  }
}
