import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TickTickCreateParams {
  title: string;
  content?: string;
  dueDate?: string;
  priority?: number;
  tags?: string[];
  listId?: string;
}

export async function execute(
  context: SkillContext,
  params: TickTickCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title) {
    return {
      success: false,
      error: 'title is required',
    };
  }

  try {
    const response = await gateway.call('ticktick.create', {
      title: params.title,
      content: params.content,
      dueDate: params.dueDate,
      priority: params.priority || 0,
      tags: params.tags || [],
      listId: params.listId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create TickTick task',
    };
  }
}
