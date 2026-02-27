import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface OverseerrRequestParams {
  mediaType: string;
  mediaId: number;
  seasons?: number[];
  is4k?: boolean;
}

export async function execute(
  context: SkillContext,
  params: OverseerrRequestParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.mediaType || !params.mediaId) {
    return {
      success: false,
      error: 'mediaType and mediaId are required',
    };
  }

  try {
    const response = await gateway.call('overseerr.request', {
      mediaType: params.mediaType,
      mediaId: params.mediaId,
      seasons: params.seasons,
      is4k: params.is4k || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Overseerr request',
    };
  }
}
