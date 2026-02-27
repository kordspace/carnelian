import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BraveSearchParams {
  query: string;
  count?: number;
  offset?: number;
  safesearch?: 'off' | 'moderate' | 'strict';
  freshness?: string;
}

export async function execute(
  context: SkillContext,
  params: BraveSearchParams
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
    const response = await gateway.call('brave.search', {
      query: params.query,
      count: params.count || 10,
      offset: params.offset || 0,
      safesearch: params.safesearch || 'moderate',
      freshness: params.freshness,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to search with Brave',
    };
  }
}
