import type { SkillContext, SkillResult } from '../../types';

interface CalendarCreateParams {
  title: string;
  start: string;
  end: string;
  description?: string;
  location?: string;
  attendees?: string[];
  reminders?: number[];
}

export async function execute(
  context: SkillContext,
  params: CalendarCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.start || !params.end) {
    return {
      success: false,
      error: 'title, start, and end are required',
    };
  }

  try {
    const response = await gateway.call('calendar.create', {
      title: params.title,
      start: params.start,
      end: params.end,
      description: params.description || '',
      location: params.location || '',
      attendees: params.attendees || [],
      reminders: params.reminders || [15],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create calendar event',
    };
  }
}
