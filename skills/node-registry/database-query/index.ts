import type { SkillContext, SkillResult } from '../../types';

interface DatabaseQueryParams {
  query: string;
  database?: string;
  connection?: string;
  params?: Record<string, unknown> | unknown[];
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: DatabaseQueryParams
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
    const response = await gateway.call('database.query', {
      query: params.query,
      database: params.database,
      connection: params.connection,
      params: params.params || [],
      timeout: params.timeout || 30000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute database query',
    };
  }
}
