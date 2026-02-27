import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface RadarrMovieParams {
  tmdbId?: number;
  title?: string;
  qualityProfileId?: number;
  rootFolderPath?: string;
}

export async function execute(
  context: SkillContext,
  params: RadarrMovieParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('radarr.movie', {
      tmdbId: params.tmdbId,
      title: params.title,
      qualityProfileId: params.qualityProfileId,
      rootFolderPath: params.rootFolderPath,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to manage Radarr movie',
    };
  }
}
