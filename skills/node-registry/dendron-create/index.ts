import type { SkillContext, SkillResult } from '../../types';

interface DendronCreateParams {
  title: string;
  content: string;
  vault?: string;
  hierarchy?: string;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: DendronCreateParams
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
    const response = await gateway.call('dendron.create', {
      title: params.title,
      content: params.content,
      vault: params.vault,
      hierarchy: params.hierarchy,
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Dendron note',
    };
  }
}
