import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface OmbiRequestParams {
  requestType: string;
  theMovieDbId?: number;
  tvDbId?: number;
  seasonRequests?: Array<{ seasonNumber: number }>;
}

export async function execute(
  context: SkillContext,
  params: OmbiRequestParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.requestType) {
    return {
      success: false,
      error: 'requestType is required',
    };
  }

  try {
    const response = await gateway.call('ombi.request', {
      requestType: params.requestType,
      theMovieDbId: params.theMovieDbId,
      tvDbId: params.tvDbId,
      seasonRequests: params.seasonRequests,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Ombi request',
    };
  }
}
