import type { SkillContext, SkillResult } from '../../types';

interface FileUploadParams {
  filePath: string;
  destination: string;
  filename?: string;
  metadata?: Record<string, unknown>;
  overwrite?: boolean;
}

export async function execute(
  context: SkillContext,
  params: FileUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.filePath || !params.destination) {
    return {
      success: false,
      error: 'filePath and destination are required',
    };
  }

  try {
    const response = await gateway.call('file.upload', {
      filePath: params.filePath,
      destination: params.destination,
      filename: params.filename,
      metadata: params.metadata || {},
      overwrite: params.overwrite !== false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload file',
    };
  }
}
