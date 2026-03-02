import type { SkillContext, SkillResult } from '../../types';

interface WebSearchParams {
  query: string;
  maxResults?: number;
  searchEngine?: string;
  region?: string;
  timeRange?: string;
}

export async function execute(
  context: SkillContext,
  params: WebSearchParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.query) {
    return {
      success: false,
      error: 'query is required',
    };
  }

  try {
    const response = await gateway.call('web.search', {
      query: params.query,
      maxResults: params.maxResults || 10,
      searchEngine: params.searchEngine || 'google',
      region: params.region,
      timeRange: params.timeRange,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to perform web search',
    };
  }
}
