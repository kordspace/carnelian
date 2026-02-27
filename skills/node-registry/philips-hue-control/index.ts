import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PhilipsHueControlParams {
  lightId: string;
  on?: boolean;
  brightness?: number;
  hue?: number;
  saturation?: number;
  transitionTime?: number;
}

export async function execute(
  context: SkillContext,
  params: PhilipsHueControlParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.lightId) {
    return {
      success: false,
      error: 'lightId is required',
    };
  }

  try {
    const response = await gateway.call('philipshue.control', {
      lightId: params.lightId,
      on: params.on,
      brightness: params.brightness,
      hue: params.hue,
      saturation: params.saturation,
      transitionTime: params.transitionTime || 4,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Philips Hue light',
    };
  }
}
