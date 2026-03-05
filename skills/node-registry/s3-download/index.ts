import type { SkillContext, SkillResult } from '../../types';

interface S3DownloadParams {
  bucket: string;
  key: string;
}

export async function execute(
  context: SkillContext,
  params: S3DownloadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.bucket || !params.key) {
    return {
      success: false,
      error: 'bucket and key are required',
    };
  }

  try {
    const response = await gateway.call('s3.download', {
      bucket: params.bucket,
      key: params.key,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to download from S3',
    };
  }
}
