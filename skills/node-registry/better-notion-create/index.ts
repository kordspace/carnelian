import type { SkillContext, SkillResult } from '../../types';

interface BetterNotionCreateParams {
  title: string;
  content: string;
  databaseId?: string;
  properties?: Record<string, any>;
  parent?: string;
}

export async function execute(
  context: SkillContext,
  params: BetterNotionCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.content) {
    return {
      success: false,
      error: 'title and content are required',
    };
  }

  try {
    const response = await gateway.call('notion.createEnhanced', {
      title: params.title,
      content: params.content,
      databaseId: params.databaseId,
      properties: params.properties || {},
      parent: params.parent,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create enhanced Notion page',
    };
  }
}
