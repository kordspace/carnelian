import type { SkillContext, SkillResult } from '../../types';

interface ThingsCreateTaskParams {
  title: string;
  notes?: string;
  when?: string;
  deadline?: string;
  tags?: string[];
  checklist?: string[];
}

export async function execute(
  context: SkillContext,
  params: ThingsCreateTaskParams
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
    const response = await gateway.call('things.createTask', {
      title: params.title,
      notes: params.notes,
      when: params.when,
      deadline: params.deadline,
      tags: params.tags || [],
      checklist: params.checklist || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Things task',
    };
  }
}
