import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GDriveListParams {
  query?: string;
  folderId?: string;
  maxResults?: number;
}

export async function execute(
  context: SkillContext,
  params: GDriveListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('gdrive.list', {
      query: params.query || '',
      folderId: params.folderId,
      maxResults: params.maxResults || 100,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list Google Drive files',
    };
  }
}
