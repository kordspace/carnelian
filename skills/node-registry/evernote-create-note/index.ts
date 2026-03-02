import type { SkillContext, SkillResult } from '../../types';

interface EvernoteCreateNoteParams {
  title: string;
  content: string;
  notebookGuid?: string;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: EvernoteCreateNoteParams
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
    const response = await gateway.call('evernote.createNote', {
      title: params.title,
      content: params.content,
      notebookGuid: params.notebookGuid,
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Evernote note',
    };
  }
}
