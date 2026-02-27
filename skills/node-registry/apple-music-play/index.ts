import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AppleMusicPlayParams {
  track?: string;
  album?: string;
  artist?: string;
  playlist?: string;
}

export async function execute(
  context: SkillContext,
  params: AppleMusicPlayParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('apple.music.play', {
      track: params.track,
      album: params.album,
      artist: params.artist,
      playlist: params.playlist,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to play Apple Music',
    };
  }
}
