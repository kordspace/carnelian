import type { SkillContext, SkillResult } from '../../types';

interface RokuControlParams {
  deviceId: string;
  command: 'home' | 'play' | 'pause' | 'select' | 'back' | 'up' | 'down' | 'left' | 'right';
  appId?: string;
}

export async function execute(
  context: SkillContext,
  params: RokuControlParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.deviceId || !params.command) {
    return {
      success: false,
      error: 'deviceId and command are required',
    };
  }

  try {
    const response = await gateway.call('roku.control', {
      deviceId: params.deviceId,
      command: params.command,
      appId: params.appId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Roku device',
    };
  }
}
