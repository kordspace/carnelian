import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AppleMusicVolumeParams {
  volume: number;
}

export async function execute(
  context: SkillContext,
  params: AppleMusicVolumeParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (params.volume === undefined || params.volume < 0 || params.volume > 100) {
    return {
      success: false,
      error: 'volume must be between 0 and 100',
    };
  }

  try {
    const response = await gateway.call('apple.music.volume', {
      volume: params.volume,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to set Apple Music volume',
    };
  }
}
