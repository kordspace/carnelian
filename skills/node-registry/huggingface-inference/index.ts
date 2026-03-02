import type { SkillContext, SkillResult } from '../../types';

interface HuggingFaceInferenceParams {
  model: string;
  inputs: any;
  parameters?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: HuggingFaceInferenceParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.model || !params.inputs) {
    return {
      success: false,
      error: 'model and inputs are required',
    };
  }

  try {
    const response = await gateway.call('huggingface.inference', {
      model: params.model,
      inputs: params.inputs,
      parameters: params.parameters || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to run HuggingFace inference',
    };
  }
}
