import type { SkillContext, SkillResult } from '../../types';

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

  if (!params.text) {
    return {
      success: false,
      error: 'text is required',
    };
  }

  try {
    const response = await fetch(`${context.gateway}/internal/edge/tts`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        text: params.text,
        voice: params.voice || 'en-US-AriaNeural',
        rate: params.rate || '+0%',
        volume: params.volume || '+0%',
        outputFormat: params.outputFormat || 'audio-24khz-48kbitrate-mono-mp3',
      }),
    });

    if (!response.ok) {
      return {
        success: false,
        error: `Edge TTS failed: ${response.statusText}`,
      };
    }

    const data = await response.json();

    return {
      success: true,
      data,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate speech with Edge TTS',
    };
  }
}
