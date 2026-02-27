import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TrelloCardParams {
  action: 'create' | 'update' | 'move' | 'get' | 'list' | 'delete';
  boardId?: string;
  listId?: string;
  cardId?: string;
  name?: string;
  desc?: string;
  due?: string;
  labels?: string[];
  pos?: 'top' | 'bottom' | number;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: TrelloCardParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('trello.card', {
      action: params.action,
      boardId: params.boardId,
      listId: params.listId,
      cardId: params.cardId,
      name: params.name,
      desc: params.desc,
      due: params.due,
      labels: params.labels,
      pos: params.pos,
      limit: params.limit || 50,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Trello card action',
    };
  }
}
