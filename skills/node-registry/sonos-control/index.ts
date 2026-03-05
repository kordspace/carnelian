import type { SkillContext, SkillResult } from '../../types';

interface SonosControlParams {
  deviceId: string;
  action: 'play' | 'pause' | 'next' | 'previous' | 'volume' | 'mute';
  value?: number;
  uri?: string;
}

export async function execute(
  context: SkillContext,
  params: SonosControlParams
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
    const response = await gateway.call('sonos.control', {
      deviceId: params.deviceId,
      action: params.action,
      value: params.value,
      uri: params.uri,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Sonos device',
    };
  }
}
