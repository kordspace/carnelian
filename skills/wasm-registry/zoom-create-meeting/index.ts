import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ZoomCreateMeetingParams {
  topic: string;
  startTime?: string;
  duration?: number;
  password?: string;
}

export async function execute(
  context: SkillContext,
  params: ZoomCreateMeetingParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.topic) {
    return {
      success: false,
      error: 'topic is required',
    };
  }

  try {
    const response = await gateway.call('zoom.createMeeting', {
      topic: params.topic,
      startTime: params.startTime,
      duration: params.duration || 60,
      password: params.password,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Zoom meeting',
    };
  }
}
