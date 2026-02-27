import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AssemblyAITranscribeParams {
  audioUrl: string;
  languageCode?: string;
  speakerLabels?: boolean;
  punctuate?: boolean;
  formatText?: boolean;
}

export async function execute(
  context: SkillContext,
  params: AssemblyAITranscribeParams
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
    const response = await gateway.call('assemblyai.transcribe', {
      audioUrl: params.audioUrl,
      languageCode: params.languageCode || 'en',
      speakerLabels: params.speakerLabels || false,
      punctuate: params.punctuate || true,
      formatText: params.formatText || true,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to transcribe with AssemblyAI',
    };
  }
}
