import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AppleTVControlParams {
  deviceId: string;
  command: 'play' | 'pause' | 'menu' | 'select' | 'up' | 'down' | 'left' | 'right';
  appId?: string;
}

export async function execute(
  context: SkillContext,
  params: AppleTVControlParams
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
    const response = await gateway.call('appletv.control', {
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
      error: error instanceof Error ? error.message : 'Failed to control Apple TV',
    };
  }
}
