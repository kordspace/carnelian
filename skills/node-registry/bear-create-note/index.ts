import type { SkillContext, SkillResult } from '../../types';

interface BearCreateNoteParams {
  title: string;
  text: string;
  tags?: string[];
  pin?: boolean;
}

export async function execute(
  context: SkillContext,
  params: BearCreateNoteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.text) {
    return {
      success: false,
      error: 'title and text are required',
    };
  }

  try {
    const response = await gateway.call('bear.createNote', {
      title: params.title,
      text: params.text,
      tags: params.tags || [],
      pin: params.pin || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Bear note',
    };
  }
}
