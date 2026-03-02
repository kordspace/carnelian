import type { SkillContext, SkillResult } from '../../types';

interface CalDAVCreateEventParams {
  summary: string;
  startDate: string;
  endDate: string;
  description?: string;
  location?: string;
  calendarUrl: string;
}

export async function execute(
  context: SkillContext,
  params: CalDAVCreateEventParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.summary || !params.startDate || !params.endDate || !params.calendarUrl) {
    return {
      success: false,
      error: 'summary, startDate, endDate, and calendarUrl are required',
    };
  }

  try {
    const response = await gateway.call('caldav.createEvent', {
      summary: params.summary,
      startDate: params.startDate,
      endDate: params.endDate,
      description: params.description,
      location: params.location,
      calendarUrl: params.calendarUrl,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create CalDAV event',
    };
  }
}
