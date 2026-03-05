import type { SkillContext, SkillResult } from '../../types';

interface NotionDatabaseParams {
  action: 'query' | 'create_page' | 'update_page' | 'get_page';
  databaseId?: string;
  pageId?: string;
  properties?: Record<string, unknown>;
  filter?: Record<string, unknown>;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: NotionDatabaseParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('notion.database', {
      action: params.action,
      databaseId: params.databaseId,
      pageId: params.pageId,
      properties: params.properties,
      filter: params.filter,
      limit: params.limit || 100,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Notion database action',
    };
  }
}
