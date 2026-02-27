import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface LogseqCreateParams {
  title: string;
  content: string;
  properties?: Record<string, string>;
  graph?: string;
}

export async function execute(
  context: SkillContext,
  params: LogseqCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.content) {
    return {
      success: false,
      error: 'title and content are required',
    };
  }

  try {
    const response = await gateway.call('logseq.create', {
      title: params.title,
      content: params.content,
      properties: params.properties || {},
      graph: params.graph,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Logseq page',
    };
  }
}
