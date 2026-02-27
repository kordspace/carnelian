import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface StravaActivityParams {
  name: string;
  type: string;
  startDate: string;
  elapsedTime: number;
  distance?: number;
  description?: string;
}

export async function execute(
  context: SkillContext,
  params: StravaActivityParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name || !params.type || !params.startDate || !params.elapsedTime) {
    return {
      success: false,
      error: 'name, type, startDate, and elapsedTime are required',
    };
  }

  try {
    const response = await gateway.call('strava.activity', {
      name: params.name,
      type: params.type,
      startDate: params.startDate,
      elapsedTime: params.elapsedTime,
      distance: params.distance,
      description: params.description,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Strava activity',
    };
  }
}
