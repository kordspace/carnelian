import type { SkillContext, SkillResult } from '../../types';

interface CohereGenerateParams {
  prompt: string;
  model?: string;
  maxTokens?: number;
  temperature?: number;
  stopSequences?: string[];
}

export async function execute(
  context: SkillContext,
  params: CohereGenerateParams
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
    const response = await gateway.call('cohere.generate', {
      prompt: params.prompt,
      model: params.model || 'command',
      maxTokens: params.maxTokens || 1024,
      temperature: params.temperature || 0.9,
      stopSequences: params.stopSequences,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate with Cohere',
    };
  }
}
