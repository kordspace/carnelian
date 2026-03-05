import type { SkillContext, SkillResult } from '../../types';

interface SonarrSeriesParams {
  tvdbId?: number;
  title?: string;
  qualityProfileId?: number;
  rootFolderPath?: string;
}

export async function execute(
  context: SkillContext,
  params: SonarrSeriesParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('sonarr.series', {
      tvdbId: params.tvdbId,
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
      error: error instanceof Error ? error.message : 'Failed to manage Sonarr series',
    };
  }
}
