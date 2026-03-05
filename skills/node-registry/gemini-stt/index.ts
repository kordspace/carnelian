import type { SkillContext, SkillResult } from '../../types';

interface GeminiSTTParams {
  audioUrl: string;
  languageCode?: string;
  model?: string;
}

export async function execute(
  context: SkillContext,
  params: GeminiSTTParams
): Promise<SkillResult> {

  if (!params.audioUrl) {
    return {
      success: false,
      error: 'audioUrl is required',
    };
  }

  try {
    const response = await fetch(`${context.gateway}/internal/gemini/stt`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        audioUrl: params.audioUrl,
        languageCode: params.languageCode || 'en-US',
        model: params.model || 'gemini-1.5-flash',
      }),
    });

    if (!response.ok) {
      return {
        success: false,
        error: `Gemini STT failed: ${response.statusText}`,
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
      error: error instanceof Error ? error.message : 'Failed to transcribe with Gemini STT',
    };
  }
}
