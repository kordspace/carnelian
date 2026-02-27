import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ExaSearchParams {
  query: string;
  numResults?: number;
  type?: 'neural' | 'keyword' | 'auto';
  category?: string;
  startPublishedDate?: string;
}

export async function execute(
  context: SkillContext,
  params: ExaSearchParams
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
    const response = await gateway.call('exa.search', {
      query: params.query,
      numResults: params.numResults || 10,
      type: params.type || 'auto',
      category: params.category,
      startPublishedDate: params.startPublishedDate,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to search with Exa',
    };
  }
}
