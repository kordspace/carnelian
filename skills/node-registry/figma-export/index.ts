import type { SkillContext, SkillResult } from '../../types';

interface FigmaExportParams {
  fileKey: string;
  nodeIds?: string[];
  format?: 'png' | 'jpg' | 'svg' | 'pdf';
  scale?: number;
}

export async function execute(
  context: SkillContext,
  params: FigmaExportParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.fileKey) {
    return {
      success: false,
      error: 'fileKey is required',
    };
  }

  try {
    const response = await gateway.call('figma.export', {
      fileKey: params.fileKey,
      nodeIds: params.nodeIds || [],
      format: params.format || 'png',
      scale: params.scale || 1,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to export from Figma',
    };
  }
}
