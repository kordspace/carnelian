import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface NotionCreatePageParams {
  parentId: string;
  title: string;
  content?: Array<{
    type: string;
    text?: string;
  }>;
  properties?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: NotionCreatePageParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.parentId || !params.title) {
    return {
      success: false,
      error: 'parentId and title are required',
    };
  }

  try {
    const response = await gateway.call('notion.createPage', {
      parentId: params.parentId,
      title: params.title,
      content: params.content || [],
      properties: params.properties || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Notion page',
    };
  }
}
