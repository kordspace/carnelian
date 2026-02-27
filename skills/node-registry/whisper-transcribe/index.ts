import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WhisperTranscribeParams {
  audioUrl?: string;
  audioFile?: string;
  language?: string;
  model?: string;
  temperature?: number;
}

export async function execute(
  context: SkillContext,
  params: WhisperTranscribeParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.audioUrl && !params.audioFile) {
    return {
      success: false,
      error: 'audioUrl or audioFile is required',
    };
  }

  try {
    const response = await gateway.call('whisper.transcribe', {
      audioUrl: params.audioUrl,
      audioFile: params.audioFile,
      language: params.language,
      model: params.model || 'whisper-1',
      temperature: params.temperature || 0,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to transcribe audio with Whisper',
    };
  }
}
