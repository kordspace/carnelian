import type { SkillContext, SkillResult } from '../../types';

interface MeetingSchedulerParams {
  title: string;
  attendees: string[];
  duration: number;
  preferredTimes?: string[];
  timezone?: string;
  buffer?: number;
}

export async function execute(
  context: SkillContext,
  params: MeetingSchedulerParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.attendees || params.attendees.length === 0 || !params.duration) {
    return {
      success: false,
      error: 'title, attendees, and duration are required',
    };
  }

  try {
    const response = await gateway.call('meeting.schedule', {
      title: params.title,
      attendees: params.attendees,
      duration: params.duration,
      preferredTimes: params.preferredTimes,
      timezone: params.timezone || 'UTC',
      buffer: params.buffer || 15,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to schedule meeting',
    };
  }
}
