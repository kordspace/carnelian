import type { SkillContext, SkillResult } from '../../types';

interface TickTickCompleteParams {
  taskId: string;
}

export async function execute(
  context: SkillContext,
  params: TickTickCompleteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.taskId) {
    return {
      success: false,
      error: 'taskId is required',
    };
  }

  try {
    const response = await gateway.call('ticktick.complete', {
      taskId: params.taskId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to complete TickTick task',
    };
  }
}
