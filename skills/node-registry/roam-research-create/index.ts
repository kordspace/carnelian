import type { SkillContext, SkillResult } from '../../types';

interface RoamResearchCreateParams {
  title: string;
  content: string;
  tags?: string[];
  date?: string;
}

export async function execute(
  context: SkillContext,
  params: RoamResearchCreateParams
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
    const response = await gateway.call('roam.create', {
      title: params.title,
      content: params.content,
      tags: params.tags || [],
      date: params.date,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Roam Research page',
    };
  }
}
