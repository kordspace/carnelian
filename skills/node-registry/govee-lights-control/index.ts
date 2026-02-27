import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GoveeLightsControlParams {
  deviceId: string;
  action: 'on' | 'off' | 'brightness' | 'color' | 'temperature';
  brightness?: number;
  color?: { r: number; g: number; b: number };
  temperature?: number;
}

export async function execute(
  context: SkillContext,
  params: GoveeLightsControlParams
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
    const response = await gateway.call('govee.control', {
      deviceId: params.deviceId,
      action: params.action,
      brightness: params.brightness,
      color: params.color,
      temperature: params.temperature,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Govee lights',
    };
  }
}
