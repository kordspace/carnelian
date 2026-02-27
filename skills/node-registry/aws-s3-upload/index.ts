import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AWSS3UploadParams {
  bucket: string;
  key: string;
  body: string | Buffer;
  contentType?: string;
  acl?: string;
}

export async function execute(
  context: SkillContext,
  params: AWSS3UploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.bucket || !params.key || !params.body) {
    return {
      success: false,
      error: 'bucket, key, and body are required',
    };
  }

  try {
    const response = await gateway.call('aws.s3.upload', {
      bucket: params.bucket,
      key: params.key,
      body: params.body,
      contentType: params.contentType,
      acl: params.acl,
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
