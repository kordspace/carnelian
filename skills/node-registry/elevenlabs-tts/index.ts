import type { SkillContext, SkillResult } from '../../types';

interface ElevenLabsTTSParams {
  text: string;
  voiceId?: string;
  modelId?: string;
  stability?: number;
  similarityBoost?: number;
}

export async function execute(
  context: SkillContext,
  params: ElevenLabsTTSParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.text) {
    return {
      success: false,
      error: 'text is required',
    };
  }

  try {
    const response = await gateway.call('elevenlabs.tts', {
      text: params.text,
      voiceId: params.voiceId || '21m00Tcm4TlvDq8ikWAM',
      modelId: params.modelId || 'eleven_monolingual_v1',
      stability: params.stability || 0.5,
      similarityBoost: params.similarityBoost || 0.75,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate speech with ElevenLabs',
    };
  }
}
