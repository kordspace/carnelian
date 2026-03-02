import type { SkillContext, SkillResult } from '../../types';

interface NewRelicCreateEventParams {
  eventType: string;
  attributes: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: NewRelicCreateEventParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.eventType || !params.attributes) {
    return {
      success: false,
      error: 'eventType and attributes are required',
    };
  }

  try {
    const response = await gateway.call('newrelic.createEvent', {
      eventType: params.eventType,
      attributes: params.attributes,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create New Relic event',
    };
  }
}
