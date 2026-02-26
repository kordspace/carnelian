import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CockroachDBQueryParams {
  query: string;
  params?: any[];
}

export async function execute(
  context: SkillContext,
  params: CockroachDBQueryParams
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
    const response = await gateway.call('cockroachdb.query', {
      query: params.query,
      params: params.params || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to query CockroachDB',
    };
  }
}
