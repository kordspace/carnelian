import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CalendarListParams {
  startDate?: string;
  endDate?: string;
  maxResults?: number;
  calendarId?: string;
}

export async function execute(
  context: SkillContext,
  params: CalendarListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('calendar.list', {
      startDate: params.startDate,
      endDate: params.endDate,
      maxResults: params.maxResults || 50,
      calendarId: params.calendarId || 'primary',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list calendar events',
    };
  }
}
