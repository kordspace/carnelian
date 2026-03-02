import type { SkillContext, SkillResult } from '../../types';

interface MongoDBInsertParams {
  collection: string;
  document: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: MongoDBInsertParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.collection || !params.document) {
    return {
      success: false,
      error: 'collection and document are required',
    };
  }

  try {
    const response = await gateway.call('mongodb.insert', {
      collection: params.collection,
      document: params.document,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to insert MongoDB document',
    };
  }
}
