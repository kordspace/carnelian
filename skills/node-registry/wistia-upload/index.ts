import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WistiaUploadParams {
  videoUrl: string;
  name: string;
  projectId?: string;
  description?: string;
}

export async function execute(
  context: SkillContext,
  params: WistiaUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.videoUrl || !params.name) {
    return {
      success: false,
      error: 'videoUrl and name are required',
    };
  }

  try {
    const response = await gateway.call('wistia.upload', {
      videoUrl: params.videoUrl,
      name: params.name,
      projectId: params.projectId,
      description: params.description,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to Wistia',
    };
  }
}
