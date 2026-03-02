import type { SkillContext, SkillResult } from '../../types';

interface ArxivSearchParams {
  query: string;
  maxResults?: number;
  sortBy?: 'relevance' | 'lastUpdatedDate' | 'submittedDate';
  sortOrder?: 'ascending' | 'descending';
}

export async function execute(
  context: SkillContext,
  params: ArxivSearchParams
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
    const response = await gateway.call('arxiv.search', {
      query: params.query,
      maxResults: params.maxResults || 10,
      sortBy: params.sortBy || 'relevance',
      sortOrder: params.sortOrder || 'descending',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to search arXiv',
    };
  }
}
