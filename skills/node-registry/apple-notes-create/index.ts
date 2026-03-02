import type { SkillContext, SkillResult } from '../../types';

interface AppleNotesCreateParams {
  title: string;
  body: string;
  folder?: string;
}

export async function execute(
  context: SkillContext,
  params: AppleNotesCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.body) {
    return {
      success: false,
      error: 'title and body are required',
    };
  }

  try {
    const response = await gateway.call('apple.notes.create', {
      title: params.title,
      body: params.body,
      folder: params.folder || 'Notes',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Apple note',
    };
  }
}
