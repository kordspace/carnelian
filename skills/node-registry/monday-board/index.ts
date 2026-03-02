import type { SkillContext, SkillResult } from '../../types';

interface MondayBoardParams {
  action: 'create_item' | 'update_item' | 'move_item' | 'get_board' | 'list_boards';
  boardId?: string;
  itemId?: string;
  groupId?: string;
  itemName?: string;
  columnValues?: Record<string, unknown>;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: MondayBoardParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('monday.board', {
      action: params.action,
      boardId: params.boardId,
      itemId: params.itemId,
      groupId: params.groupId,
      itemName: params.itemName,
      columnValues: params.columnValues,
      limit: params.limit || 25,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Monday.com board action',
    };
  }
}
