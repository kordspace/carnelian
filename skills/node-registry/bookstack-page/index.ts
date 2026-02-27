import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BookStackPageParams {
  action: 'create' | 'update' | 'get' | 'delete';
  bookId?: number;
  chapterId?: number;
  pageId?: number;
  name?: string;
  html?: string;
  markdown?: string;
}

export async function execute(
  context: SkillContext,
  params: BookStackPageParams
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
    const response = await gateway.call('bookstack.page', {
      action: params.action,
      bookId: params.bookId,
      chapterId: params.chapterId,
      pageId: params.pageId,
      name: params.name,
      html: params.html,
      markdown: params.markdown,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute BookStack action',
    };
  }
}
