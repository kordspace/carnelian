import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface DropboxDownloadParams {
  path: string;
}

export async function execute(
  context: SkillContext,
  params: DropboxDownloadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.path) {
    return {
      success: false,
      error: 'path is required',
    };
  }

  try {
    const response = await gateway.call('dropbox.download', {
      path: params.path,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to download from Dropbox',
    };
  }
}
