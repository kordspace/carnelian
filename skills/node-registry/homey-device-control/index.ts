import type { SkillContext, SkillResult } from '../../types';

interface HomeyDeviceControlParams {
  deviceId: string;
  action: 'on' | 'off' | 'toggle' | 'dim' | 'set';
  value?: number;
  capability?: string;
}

export async function execute(
  context: SkillContext,
  params: HomeyDeviceControlParams
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
    const response = await gateway.call('homey.device.control', {
      deviceId: params.deviceId,
      action: params.action,
      value: params.value,
      capability: params.capability || 'onoff',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Homey device',
    };
  }
}
