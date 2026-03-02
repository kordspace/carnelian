import type { SkillContext, SkillResult } from '../../types';

interface ScheduleCreateParams {
  name: string;
  cron: string;
  skill: string;
  params?: Record<string, unknown>;
  enabled?: boolean;
}

export async function execute(
  context: SkillContext,
  params: ScheduleCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name || !params.cron || !params.skill) {
    return {
      success: false,
      error: 'name, cron, and skill are required',
    };
  }

  try {
    const response = await gateway.call('schedule.create', {
      name: params.name,
      cron: params.cron,
      skill: params.skill,
      params: params.params || {},
      enabled: params.enabled !== false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create schedule',
    };
  }
}
