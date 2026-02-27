import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface NanoleafControlParams {
  deviceId: string;
  action: 'on' | 'off' | 'brightness' | 'color' | 'effect';
  brightness?: number;
  hue?: number;
  saturation?: number;
  effectName?: string;
}

export async function execute(
  context: SkillContext,
  params: NanoleafControlParams
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
    const response = await gateway.call('nanoleaf.control', {
      deviceId: params.deviceId,
      action: params.action,
      brightness: params.brightness,
      hue: params.hue,
      saturation: params.saturation,
      effectName: params.effectName,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Nanoleaf',
    };
  }
}
