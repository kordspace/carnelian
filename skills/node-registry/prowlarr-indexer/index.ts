import type { SkillContext, SkillResult } from '../../types';

interface ProwlarrIndexerParams {
  query?: string;
  categories?: number[];
  indexerIds?: number[];
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: ProwlarrIndexerParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('prowlarr.indexer', {
      query: params.query,
      categories: params.categories,
      indexerIds: params.indexerIds,
      limit: params.limit || 100,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to query Prowlarr indexers',
    };
  }
}
