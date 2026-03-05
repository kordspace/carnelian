import type { SkillContext, SkillResult } from '../../types';

interface YouTubeUploadParams {
  title: string;
  description?: string;
  videoFile: string;
  privacy?: 'public' | 'private' | 'unlisted';
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: YouTubeUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.videoFile) {
    return {
      success: false,
      error: 'title and videoFile are required',
    };
  }

  try {
    const response = await gateway.call('youtube.upload', {
      title: params.title,
      description: params.description || '',
      videoFile: params.videoFile,
      privacy: params.privacy || 'private',
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to YouTube',
    };
  }
}
