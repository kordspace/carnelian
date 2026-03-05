import type { SkillContext, SkillResult } from '../../types';

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
  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await fetch(`${context.gateway}/internal/bookstack/page`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        action: params.action,
        bookId: params.bookId,
        chapterId: params.chapterId,
        pageId: params.pageId,
        name: params.name,
        html: params.html,
        markdown: params.markdown,
      }),
    });

    if (!response.ok) {
      return {
        success: false,
        error: `BookStack operation failed: ${response.statusText}`,
      };
    }

    const data = await response.json();
    return {
      success: true,
      data,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute BookStack action',
    };
  }
}
