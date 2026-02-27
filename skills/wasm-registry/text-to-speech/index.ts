import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TextToSpeechParams {
  text: string;
  voice?: string;
  model?: string;
  speed?: number;
  format?: string;
}

export async function execute(
  context: SkillContext,
  params: TextToSpeechParams
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
    const response = await gateway.call('tts.generate', {
      text: params.text,
      voice: params.voice || 'alloy',
      model: params.model || 'tts-1',
      speed: params.speed || 1.0,
      format: params.format || 'mp3',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate speech',
    };
  }
}
