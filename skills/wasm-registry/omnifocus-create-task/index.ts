import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface OmniFocusCreateTaskParams {
  name: string;
  note?: string;
  project?: string;
  context?: string;
  dueDate?: string;
  flagged?: boolean;
}

export async function execute(
  context: SkillContext,
  params: OmniFocusCreateTaskParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name) {
    return {
      success: false,
      error: 'name is required',
    };
  }

  try {
    const response = await gateway.call('omnifocus.createTask', {
      name: params.name,
      note: params.note,
      project: params.project,
      context: params.context,
      dueDate: params.dueDate,
      flagged: params.flagged || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create OmniFocus task',
    };
  }
}
