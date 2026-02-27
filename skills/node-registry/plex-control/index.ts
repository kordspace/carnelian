import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PlexControlParams {
  action: string;
  sessionId?: string;
  mediaKey?: string;
  parameters?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: PlexControlParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('plex.control', {
      action: params.action,
      sessionId: params.sessionId,
      mediaKey: params.mediaKey,
      parameters: params.parameters || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Plex',
    };
  }
}
