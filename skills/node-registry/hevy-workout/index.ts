import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface HevyWorkoutParams {
  action: 'list' | 'create' | 'get';
  workoutId?: string;
  exercises?: Array<{ name: string; sets: number; reps: number; weight: number }>;
  startDate?: string;
  endDate?: string;
}

export async function execute(
  context: SkillContext,
  params: HevyWorkoutParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('hevy.workout', {
      action: params.action,
      workoutId: params.workoutId,
      exercises: params.exercises,
      startDate: params.startDate,
      endDate: params.endDate,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Hevy workout action',
    };
  }
}
