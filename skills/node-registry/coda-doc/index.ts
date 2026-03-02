import type { SkillContext, SkillResult } from '../../types';

interface CodaDocParams {
  action: 'create_page' | 'update_page' | 'get_page' | 'list_pages' | 'delete_page';
  docId?: string;
  pageId?: string;
  title?: string;
  content?: string;
  parentId?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: CodaDocParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('coda.doc', {
      action: params.action,
      docId: params.docId,
      pageId: params.pageId,
      title: params.title,
      content: params.content,
      parentId: params.parentId,
      limit: params.limit || 100,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute Coda doc action' };
  }
}
