import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AmplitudeTrackParams {
  userId: string;
  eventType: string;
  eventProperties?: Record<string, any>;
  userProperties?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: AmplitudeTrackParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.userId || !params.eventType) {
    return {
      success: false,
      error: 'userId and eventType are required',
    };
  }

  try {
    const response = await gateway.call('amplitude.track', {
      userId: params.userId,
      eventType: params.eventType,
      eventProperties: params.eventProperties || {},
      userProperties: params.userProperties || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to track Amplitude event',
    };
  }
}
