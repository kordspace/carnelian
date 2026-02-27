import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GoogleCalendarCreateParams {
  summary: string;
  startTime: string;
  endTime: string;
  description?: string;
  location?: string;
  attendees?: string[];
  timeZone?: string;
}

export async function execute(
  context: SkillContext,
  params: GoogleCalendarCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.summary || !params.startTime || !params.endTime) {
    return {
      success: false,
      error: 'summary, startTime, and endTime are required',
    };
  }

  try {
    const response = await gateway.call('googleCalendar.create', {
      summary: params.summary,
      startTime: params.startTime,
      endTime: params.endTime,
      description: params.description,
      location: params.location,
      attendees: params.attendees || [],
      timeZone: params.timeZone || 'UTC',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Google Calendar event',
    };
  }
}
