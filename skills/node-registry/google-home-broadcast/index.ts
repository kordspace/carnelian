import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GoogleHomeBroadcastParams {
  message: string;
  deviceName?: string;
  language?: string;
}

export async function execute(
  context: SkillContext,
  params: GoogleHomeBroadcastParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.message) {
    return {
      success: false,
      error: 'message is required',
    };
  }

  try {
    const response = await gateway.call('googlehome.broadcast', {
      message: params.message,
      deviceName: params.deviceName,
      language: params.language || 'en-US',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to broadcast to Google Home',
    };
  }
}
