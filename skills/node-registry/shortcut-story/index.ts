import type { SkillContext, SkillResult } from '../../types';

interface ShortcutStoryParams {
  action: 'create' | 'update' | 'get' | 'search' | 'change_state';
  name?: string;
  storyType?: 'feature' | 'bug' | 'chore';
  description?: string;
  workflowStateId?: string;
  storyId?: string;
  ownerIds?: string[];
  projectId?: string;
  labels?: string[];
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: ShortcutStoryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('shortcut.story', {
      action: params.action,
      name: params.name,
      storyType: params.storyType,
      description: params.description,
      workflowStateId: params.workflowStateId,
      storyId: params.storyId,
      ownerIds: params.ownerIds,
      projectId: params.projectId,
      labels: params.labels,
      limit: params.limit || 25,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute Shortcut story action' };
  }
}
