import type { SkillContext, SkillResult } from '../../types';

interface SoundCloudUploadParams {
  title: string;
  audioFile: string;
  description?: string;
  genre?: string;
  tags?: string[];
  sharing?: 'public' | 'private';
}

export async function execute(
  context: SkillContext,
  params: SoundCloudUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.audioFile) {
    return {
      success: false,
      error: 'title and audioFile are required',
    };
  }

  try {
    const response = await gateway.call('soundcloud.upload', {
      title: params.title,
      audioFile: params.audioFile,
      description: params.description,
      genre: params.genre,
      tags: params.tags || [],
      sharing: params.sharing || 'public',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to SoundCloud',
    };
  }
}
