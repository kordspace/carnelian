import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GeminiSTTParams {
  audioUrl: string;
  languageCode?: string;
  model?: string;
}

export async function execute(
  context: SkillContext,
  params: GeminiSTTParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.audioUrl) {
    return {
      success: false,
      error: 'audioUrl is required',
    };
  }

  try {
    const response = await gateway.call('gemini.stt', {
      audioUrl: params.audioUrl,
      languageCode: params.languageCode || 'en-US',
      model: params.model || 'gemini-1.5-flash',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to transcribe with Gemini STT',
    };
  }
}
