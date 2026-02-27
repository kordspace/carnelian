import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ICloudDriveListParams {
  path?: string;
  recursive?: boolean;
  fileType?: string;
}

export async function execute(
  context: SkillContext,
  params: ICloudDriveListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('icloud.list', {
      path: params.path || '/',
      recursive: params.recursive || false,
      fileType: params.fileType,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list iCloud Drive files',
    };
  }
}
