import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface OutlookCalendarCreateParams {
  subject: string;
  startDateTime: string;
  endDateTime: string;
  body?: string;
  location?: string;
  attendees?: string[];
}

export async function execute(
  context: SkillContext,
  params: OutlookCalendarCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.subject || !params.startDateTime || !params.endDateTime) {
    return {
      success: false,
      error: 'subject, startDateTime, and endDateTime are required',
    };
  }

  try {
    const response = await gateway.call('outlookCalendar.create', {
      subject: params.subject,
      startDateTime: params.startDateTime,
      endDateTime: params.endDateTime,
      body: params.body,
      location: params.location,
      attendees: params.attendees || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Outlook Calendar event',
    };
  }
}
