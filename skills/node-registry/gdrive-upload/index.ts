import type { SkillContext, SkillResult } from '../../types';

interface GDriveUploadParams {
  name: string;
  content: string;
  mimeType?: string;
  folderId?: string;
}

export async function execute(
  context: SkillContext,
  params: GDriveUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name || !params.content) {
    return {
      success: false,
      error: 'name and content are required',
    };
  }

  try {
    const response = await gateway.call('gdrive.upload', {
      name: params.name,
      content: params.content,
      mimeType: params.mimeType || 'text/plain',
      folderId: params.folderId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to Google Drive',
    };
  }
}
