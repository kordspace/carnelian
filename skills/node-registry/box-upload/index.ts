import type { SkillContext, SkillResult } from '../../types';

interface BoxUploadParams {
  filePath: string;
  folderId: string;
  fileName?: string;
  description?: string;
}

export async function execute(
  context: SkillContext,
  params: BoxUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.filePath || !params.folderId) {
    return {
      success: false,
      error: 'filePath and folderId are required',
    };
  }

  try {
    const response = await gateway.call('box.upload', {
      filePath: params.filePath,
      folderId: params.folderId,
      fileName: params.fileName,
      description: params.description,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to Box',
    };
  }
}
