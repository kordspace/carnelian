import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GooglePaLMGenerateParams {
  prompt: string;
  model?: string;
  temperature?: number;
  maxOutputTokens?: number;
  topP?: number;
  topK?: number;
}

export async function execute(
  context: SkillContext,
  params: GooglePaLMGenerateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.prompt) {
    return {
      success: false,
      error: 'prompt is required',
    };
  }

  try {
    const response = await gateway.call('googlepalm.generate', {
      prompt: params.prompt,
      model: params.model || 'text-bison-001',
      temperature: params.temperature || 0.7,
      maxOutputTokens: params.maxOutputTokens || 1024,
      topP: params.topP || 0.95,
      topK: params.topK || 40,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate with Google PaLM',
    };
  }
}
