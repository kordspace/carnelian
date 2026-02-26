import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ObsidianCreateNoteParams {
  vault: string;
  path: string;
  content: string;
  frontmatter?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: ObsidianCreateNoteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.vault || !params.path || !params.content) {
    return {
      success: false,
      error: 'vault, path, and content are required',
    };
  }

  try {
    const response = await gateway.call('obsidian.createNote', {
      vault: params.vault,
      path: params.path,
      content: params.content,
      frontmatter: params.frontmatter || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Obsidian note',
    };
  }
}
