import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TrelloCreateCardParams {
  listId: string;
  name: string;
  desc?: string;
  due?: string;
  labels?: string[];
}

export async function execute(
  context: SkillContext,
  params: TrelloCreateCardParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.listId || !params.name) {
    return {
      success: false,
      error: 'listId and name are required',
    };
  }

  try {
    const response = await gateway.call('trello.createCard', {
      listId: params.listId,
      name: params.name,
      desc: params.desc || '',
      due: params.due,
      labels: params.labels || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Trello card',
    };
  }
}
