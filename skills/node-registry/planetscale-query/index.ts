import type { SkillContext, SkillResult } from '../../types';

interface PlanetScaleQueryParams {
  database: string;
  query: string;
  params?: any[];
}

export async function execute(
  context: SkillContext,
  params: PlanetScaleQueryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.database || !params.query) {
    return {
      success: false,
      error: 'database and query are required',
    };
  }

  try {
    const response = await gateway.call('planetscale.query', {
      database: params.database,
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
      error: error instanceof Error ? error.message : 'Failed to query PlanetScale',
    };
  }
}
