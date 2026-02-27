import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface NotionQueryDatabaseParams {
  databaseId: string;
  filter?: Record<string, any>;
  sorts?: Array<{
    property: string;
    direction: 'ascending' | 'descending';
  }>;
  pageSize?: number;
}

export async function execute(
  context: SkillContext,
  params: NotionQueryDatabaseParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.databaseId) {
    return {
      success: false,
      error: 'databaseId is required',
    };
  }

  try {
    const response = await gateway.call('notion.queryDatabase', {
      databaseId: params.databaseId,
      filter: params.filter,
      sorts: params.sorts || [],
      pageSize: params.pageSize || 100,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to query Notion database',
    };
  }
}
