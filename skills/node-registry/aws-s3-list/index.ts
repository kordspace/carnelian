import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AWSS3ListParams {
  bucket: string;
  prefix?: string;
  maxKeys?: number;
}

export async function execute(
  context: SkillContext,
  params: AWSS3ListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.bucket) {
    return {
      success: false,
      error: 'bucket is required',
    };
  }

  try {
    const response = await gateway.call('aws.s3List', {
      bucket: params.bucket,
      prefix: params.prefix || '',
      maxKeys: params.maxKeys || 1000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list AWS S3 objects',
    };
  }
}
