import type { SkillContext, SkillResult } from '../../types';

interface FileDownloadParams {
  url: string;
  destination: string;
  filename?: string;
  timeout?: number;
  headers?: Record<string, string>;
}

export async function execute(
  context: SkillContext,
  params: FileDownloadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.url || !params.destination) {
    return {
      success: false,
      error: 'url and destination are required',
    };
  }

  try {
    const response = await gateway.call('file.download', {
      url: params.url,
      destination: params.destination,
      filename: params.filename,
      timeout: params.timeout || 60000,
      headers: params.headers || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to download file',
    };
  }
}
