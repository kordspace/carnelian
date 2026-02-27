import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TaskScheduleParams {
  task: string;
  schedule: string;
  params?: Record<string, unknown>;
  enabled?: boolean;
  timezone?: string;
}

export async function execute(
  context: SkillContext,
  params: TaskScheduleParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.task || !params.schedule) {
    return {
      success: false,
      error: 'task and schedule are required',
    };
  }

  try {
    const response = await gateway.call('task.schedule', {
      task: params.task,
      schedule: params.schedule,
      params: params.params || {},
      enabled: params.enabled !== false,
      timezone: params.timezone || 'UTC',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to schedule task',
    };
  }
}
