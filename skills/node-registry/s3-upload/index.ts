import type { SkillContext, SkillResult } from '../../types';

interface S3UploadParams {
  bucket: string;
  key: string;
  content: string;
  contentType?: string;
  acl?: 'private' | 'public-read' | 'public-read-write';
}

export async function execute(
  context: SkillContext,
  params: S3UploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.bucket || !params.key || !params.content) {
    return {
      success: false,
      error: 'bucket, key, and content are required',
    };
  }

  try {
    const response = await gateway.call('s3.upload', {
      bucket: params.bucket,
      key: params.key,
      content: params.content,
      contentType: params.contentType || 'application/octet-stream',
      acl: params.acl || 'private',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to S3',
    };
  }
}
