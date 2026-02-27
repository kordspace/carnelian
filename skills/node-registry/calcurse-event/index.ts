import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CalcurseEventParams {
  action: 'add' | 'list' | 'delete';
  date?: string;
  time?: string;
  description?: string;
  eventId?: string;
}

export async function execute(
  context: SkillContext,
  params: CalcurseEventParams
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

  if (params.action === 'add' && (!params.date || !params.description)) {
    return {
      success: false,
      error: 'date and description are required for add action',
    };
  }

  try {
    const response = await gateway.call('calcurse.event', {
      action: params.action,
      date: params.date,
      time: params.time,
      description: params.description,
      eventId: params.eventId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute calcurse action',
    };
  }
}
