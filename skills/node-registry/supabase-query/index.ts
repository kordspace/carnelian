import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SupabaseQueryParams {
  table: string;
  select?: string;
  filter?: Record<string, any>;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: SupabaseQueryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.table) {
    return {
      success: false,
      error: 'table is required',
    };
  }

  try {
    const response = await gateway.call('supabase.query', {
      table: params.table,
      select: params.select || '*',
      filter: params.filter || {},
      limit: params.limit,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to query Supabase',
    };
  }
}
