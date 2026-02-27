import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AnalyticsTrackParams {
  event: string;
  userId?: string;
  properties?: Record<string, any>;
  timestamp?: number;
}

export async function execute(
  context: SkillContext,
  params: AnalyticsTrackParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.event) {
    return {
      success: false,
      error: 'event is required',
    };
  }

  try {
    const response = await gateway.call('analytics.track', {
      event: params.event,
      userId: params.userId,
      properties: params.properties || {},
      timestamp: params.timestamp || Date.now(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to track analytics event',
    };
  }
}
