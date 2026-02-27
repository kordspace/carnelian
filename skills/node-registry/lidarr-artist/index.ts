import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface LidarrArtistParams {
  foreignArtistId?: string;
  artistName?: string;
  qualityProfileId?: number;
  metadataProfileId?: number;
  rootFolderPath?: string;
}

export async function execute(
  context: SkillContext,
  params: LidarrArtistParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('lidarr.artist', {
      foreignArtistId: params.foreignArtistId,
      artistName: params.artistName,
      qualityProfileId: params.qualityProfileId,
      metadataProfileId: params.metadataProfileId,
      rootFolderPath: params.rootFolderPath,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to manage Lidarr artist',
    };
  }
}
