import type { SkillContext, SkillResult } from '../../types';

interface MixpanelTrackParams {
  event: string;
  distinctId: string;
  properties?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: MixpanelTrackParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.event || !params.distinctId) {
    return {
      success: false,
      error: 'event and distinctId are required',
    };
  }

  try {
    const response = await gateway.call('mixpanel.track', {
      event: params.event,
      distinctId: params.distinctId,
      properties: params.properties || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to track Mixpanel event',
    };
  }
}
