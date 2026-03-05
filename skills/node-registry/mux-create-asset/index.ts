import type { SkillContext, SkillResult } from '../../types';

interface MuxCreateAssetParams {
  url: string;
  playbackPolicy?: 'public' | 'signed';
  mp4Support?: 'standard' | 'none';
}

export async function execute(
  context: SkillContext,
  params: MuxCreateAssetParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.url) {
    return {
      success: false,
      error: 'url is required',
    };
  }

  try {
    const response = await gateway.call('mux.createAsset', {
      url: params.url,
      playbackPolicy: params.playbackPolicy || 'public',
      mp4Support: params.mp4Support || 'standard',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Mux asset',
    };
  }
}
