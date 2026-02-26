import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SegmentTrackParams {
  userId: string;
  event: string;
  properties?: Record<string, any>;
  context?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: SegmentTrackParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.userId || !params.event) {
    return {
      success: false,
      error: 'userId and event are required',
    };
  }

  try {
    const response = await gateway.call('segment.track', {
      userId: params.userId,
      event: params.event,
      properties: params.properties || {},
      context: params.context || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to track Segment event',
    };
  }
}
