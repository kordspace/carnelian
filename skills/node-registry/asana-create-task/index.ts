import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AsanaCreateTaskParams {
  workspace: string;
  name: string;
  notes?: string;
  assignee?: string;
  dueOn?: string;
  projects?: string[];
}

export async function execute(
  context: SkillContext,
  params: AsanaCreateTaskParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.workspace || !params.name) {
    return {
      success: false,
      error: 'workspace and name are required',
    };
  }

  try {
    const response = await gateway.call('asana.createTask', {
      workspace: params.workspace,
      name: params.name,
      notes: params.notes || '',
      assignee: params.assignee,
      dueOn: params.dueOn,
      projects: params.projects || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Asana task',
    };
  }
}
