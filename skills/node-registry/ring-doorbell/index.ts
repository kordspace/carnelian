import type { SkillContext, SkillResult } from '../../types';

interface RingDoorbellParams {
  deviceId: string;
  action: string;
  duration?: number;
}

export async function execute(
  context: SkillContext,
  params: RingDoorbellParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.deviceId || !params.action) {
    return {
      success: false,
      error: 'deviceId and action are required',
    };
  }

  try {
    const response = await gateway.call('ring.doorbell', {
      deviceId: params.deviceId,
      action: params.action,
      duration: params.duration,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Ring doorbell',
    };
  }
}
