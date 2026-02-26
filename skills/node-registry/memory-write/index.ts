import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface MemoryWriteParams {
  content: string;
  key?: string;
  tags?: string[];
  metadata?: Record<string, unknown>;
}

export async function execute(
  context: SkillContext,
  params: MemoryWriteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.content) {
    return {
      success: false,
      error: 'content is required',
    };
  }

  try {
    const response = await gateway.call('memory.write', {
      content: params.content,
      key: params.key,
      tags: params.tags || [],
      metadata: params.metadata || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to write memory',
    };
  }
}
