import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface HeartbeatSendParams {
  serviceId: string;
  status?: string;
  metadata?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: HeartbeatSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.serviceId) {
    return {
      success: false,
      error: 'serviceId is required',
    };
  }

  try {
    const response = await gateway.call('heartbeat.send', {
      serviceId: params.serviceId,
      status: params.status || 'healthy',
      metadata: params.metadata || {},
      timestamp: Date.now(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send heartbeat',
    };
  }
}
