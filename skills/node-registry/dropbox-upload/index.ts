import type { SkillContext, SkillResult } from '../../types';

interface DropboxUploadParams {
  path: string;
  content: string;
  mode?: 'add' | 'overwrite' | 'update';
  autorename?: boolean;
}

export async function execute(
  context: SkillContext,
  params: DropboxUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.path || !params.content) {
    return {
      success: false,
      error: 'path and content are required',
    };
  }

  try {
    const response = await gateway.call('dropbox.upload', {
      path: params.path,
      content: params.content,
      mode: params.mode || 'add',
      autorename: params.autorename || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to Dropbox',
    };
  }
}
