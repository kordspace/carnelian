import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AppleCalendarCreateParams {
  title: string;
  startDate: string;
  endDate: string;
  notes?: string;
  location?: string;
  calendar?: string;
}

export async function execute(
  context: SkillContext,
  params: AppleCalendarCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.startDate || !params.endDate) {
    return {
      success: false,
      error: 'title, startDate, and endDate are required',
    };
  }

  try {
    const response = await gateway.call('appleCalendar.create', {
      title: params.title,
      startDate: params.startDate,
      endDate: params.endDate,
      notes: params.notes,
      location: params.location,
      calendar: params.calendar,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Apple Calendar event',
    };
  }
}
