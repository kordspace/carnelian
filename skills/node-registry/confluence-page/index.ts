import type { SkillContext, SkillResult } from '../../types';

interface ConfluencePageParams {
  action: 'create' | 'update' | 'get' | 'search' | 'delete';
  spaceKey?: string;
  title?: string;
  content?: string;
  pageId?: string;
  parentId?: string;
  searchQuery?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: ConfluencePageParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('confluence.page', {
      action: params.action,
      spaceKey: params.spaceKey,
      title: params.title,
      content: params.content,
      pageId: params.pageId,
      parentId: params.parentId,
      searchQuery: params.searchQuery,
      limit: params.limit || 25,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Confluence page action',
    };
  }
}
