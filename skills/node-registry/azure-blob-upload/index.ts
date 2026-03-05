import type { SkillContext, SkillResult } from '../../types';

interface AzureBlobUploadParams {
  containerName: string;
  blobName: string;
  content: string | Buffer;
  contentType?: string;
  metadata?: Record<string, string>;
}

export async function execute(
  context: SkillContext,
  params: AzureBlobUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.containerName || !params.blobName || !params.content) {
    return {
      success: false,
      error: 'containerName, blobName, and content are required',
    };
  }

  try {
    const response = await gateway.call('azure.blob.upload', {
      containerName: params.containerName,
      blobName: params.blobName,
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
      error: error instanceof Error ? error.message : 'Failed to upload to Azure Blob Storage',
    };
  }
}
