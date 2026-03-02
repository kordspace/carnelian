import type { SkillContext, SkillResult } from '../../types';

interface KnowledgeGraphLinkParams {
  sourceNode: string;
  targetNode: string;
  relationship: string;
  bidirectional?: boolean;
  metadata?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: KnowledgeGraphLinkParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.sourceNode || !params.targetNode || !params.relationship) {
    return {
      success: false,
      error: 'sourceNode, targetNode, and relationship are required',
    };
  }

  try {
    const response = await gateway.call('knowledge.link', {
      sourceNode: params.sourceNode,
      targetNode: params.targetNode,
      relationship: params.relationship,
      bidirectional: params.bidirectional || false,
      metadata: params.metadata || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create knowledge graph link',
    };
  }
}
