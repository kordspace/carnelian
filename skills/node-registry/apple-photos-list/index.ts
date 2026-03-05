import type { SkillContext, SkillResult } from '../../types';

interface ApplePhotosListParams {
  album?: string;
  limit?: number;
  startDate?: string;
  endDate?: string;
}

export async function execute(
  context: SkillContext,
  params: ApplePhotosListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('apple.photos.list', {
      album: params.album,
      limit: params.limit || 100,
      startDate: params.startDate,
      endDate: params.endDate,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list Apple photos',
    };
  }
}
