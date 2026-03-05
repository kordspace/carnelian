import type { SkillContext, SkillResult } from '../../types';

interface SmartThingsDeviceParams {
  deviceId: string;
  capability: string;
  command: string;
  arguments?: any[];
}

export async function execute(
  context: SkillContext,
  params: SmartThingsDeviceParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.deviceId || !params.capability || !params.command) {
    return {
      success: false,
      error: 'deviceId, capability, and command are required',
    };
  }

  try {
    const response = await gateway.call('smartthings.device', {
      deviceId: params.deviceId,
      capability: params.capability,
      command: params.command,
      arguments: params.arguments || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control SmartThings device',
    };
  }
}
