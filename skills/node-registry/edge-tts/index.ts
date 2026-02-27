import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface EdgeTTSParams {
  text: string;
  voice?: string;
  rate?: string;
  volume?: string;
  outputFormat?: string;
}

export async function execute(
  context: SkillContext,
  params: EdgeTTSParams
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
    const response = await gateway.call('edge.tts', {
      text: params.text,
      voice: params.voice || 'en-US-AriaNeural',
      rate: params.rate || '+0%',
      volume: params.volume || '+0%',
      outputFormat: params.outputFormat || 'audio-24khz-48kbitrate-mono-mp3',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate speech with Edge TTS',
    };
  }
}
