import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ICloudDriveUploadParams {
  filePath: string;
  destinationPath: string;
  fileName?: string;
  overwrite?: boolean;
}

export async function execute(
  context: SkillContext,
  params: ICloudDriveUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.filePath || !params.destinationPath) {
    return {
      success: false,
      error: 'filePath and destinationPath are required',
    };
  }

  try {
    const response = await gateway.call('icloud.upload', {
      filePath: params.filePath,
      destinationPath: params.destinationPath,
      fileName: params.fileName,
      overwrite: params.overwrite || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to iCloud Drive',
    };
  }
}
