import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GeminiDeepResearchParams {
  query: string;
  maxSources?: number;
  depth?: 'shallow' | 'medium' | 'deep';
  model?: string;
}

export async function execute(
  context: SkillContext,
  params: GeminiDeepResearchParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.query) {
    return {
      success: false,
      error: 'query is required',
    };
  }

  try {
    const response = await gateway.call('gemini.deepResearch', {
      query: params.query,
      maxSources: params.maxSources || 10,
      depth: params.depth || 'medium',
      model: params.model || 'gemini-2.0-flash-thinking-exp',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Gemini deep research',
    };
  }
}
