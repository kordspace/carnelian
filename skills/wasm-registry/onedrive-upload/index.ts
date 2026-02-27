import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface OneDriveUploadParams {
  path: string;
  content: string;
  conflictBehavior?: 'rename' | 'replace' | 'fail';
}

export async function execute(
  context: SkillContext,
  params: OneDriveUploadParams
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
    const response = await gateway.call('onedrive.upload', {
      path: params.path,
      content: params.content,
      conflictBehavior: params.conflictBehavior || 'rename',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to OneDrive',
    };
  }
}
