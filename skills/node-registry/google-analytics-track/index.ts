import type { SkillContext, SkillResult } from '../../types';

interface GoogleAnalyticsTrackParams {
  measurementId: string;
  clientId: string;
  events: Array<{
    name: string;
    params?: Record<string, any>;
  }>;
}

export async function execute(
  context: SkillContext,
  params: GoogleAnalyticsTrackParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.measurementId || !params.clientId || !params.events || params.events.length === 0) {
    return {
      success: false,
      error: 'measurementId, clientId, and events are required',
    };
  }

  try {
    const response = await gateway.call('googleAnalytics.track', {
      measurementId: params.measurementId,
      clientId: params.clientId,
      events: params.events,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to track Google Analytics event',
    };
  }
}
