import type { SkillContext, SkillResult } from '../../types';

interface SlackFileParams {
  action: 'upload' | 'list' | 'info' | 'delete';
  accountId?: string;
  channelId?: string;
  fileId?: string;
  content?: string;
  filename?: string;
  title?: string;
  filetype?: string;
}

export async function execute(
  context: SkillContext,
  params: SlackFileParams
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

  if (params.action === 'upload' && !params.content) {
    return {
      success: false,
      error: 'content is required for upload action',
    };
  }

  try {
    const response = await gateway.call('slack.file', {
      action: params.action,
      accountId: params.accountId,
      channelId: params.channelId,
      fileId: params.fileId,
      content: params.content,
      filename: params.filename,
      title: params.title,
      filetype: params.filetype,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to perform Slack file action',
    };
  }
}
