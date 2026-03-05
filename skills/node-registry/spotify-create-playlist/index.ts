import type { SkillContext, SkillResult } from '../../types';

interface SpotifyCreatePlaylistParams {
  name: string;
  description?: string;
  public?: boolean;
  trackUris?: string[];
}

export async function execute(
  context: SkillContext,
  params: SpotifyCreatePlaylistParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name) {
    return {
      success: false,
      error: 'name is required',
    };
  }

  try {
    const response = await gateway.call('spotify.createPlaylist', {
      name: params.name,
      description: params.description || '',
      public: params.public || false,
      trackUris: params.trackUris || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Spotify playlist',
    };
  }
}
