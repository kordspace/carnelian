import type { SkillContext, SkillResult } from '../../types';

interface EventEmitParams {
  event: string;
  data: unknown;
  channel?: string;
  broadcast?: boolean;
}

export async function execute(
  context: SkillContext,
  params: EventEmitParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.event || params.data === undefined) {
    return {
      success: false,
      error: 'event and data are required',
    };
  }

  try {
    const response = await gateway.call('event.emit', {
      event: params.event,
      data: params.data,
      channel: params.channel || 'default',
      broadcast: params.broadcast || false,
      timestamp: new Date().toISOString(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to emit event',
    };
  }
}
