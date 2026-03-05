import type { SkillContext, SkillResult } from '../../types';

interface ElasticsearchIndexParams {
  index: string;
  document: Record<string, any>;
  id?: string;
}

export async function execute(
  context: SkillContext,
  params: ElasticsearchIndexParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.index || !params.document) {
    return {
      success: false,
      error: 'index and document are required',
    };
  }

  try {
    const response = await gateway.call('elasticsearch.index', {
      index: params.index,
      document: params.document,
      id: params.id,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to index Elasticsearch document',
    };
  }
}
