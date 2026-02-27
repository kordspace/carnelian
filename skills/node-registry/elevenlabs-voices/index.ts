import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ElevenLabsVoicesParams {
  action?: 'list' | 'get' | 'create';
  voiceId?: string;
  name?: string;
  files?: string[];
}

export async function execute(
  context: SkillContext,
  params: ElevenLabsVoicesParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('elevenlabs.voices', {
      action: params.action || 'list',
      voiceId: params.voiceId,
      name: params.name,
      files: params.files,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute ElevenLabs voices action',
    };
  }
}
