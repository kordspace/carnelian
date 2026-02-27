import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface MailchimpCampaignParams {
  action: 'create' | 'send' | 'list' | 'get' | 'delete';
  campaignId?: string;
  listId?: string;
  subject?: string;
  fromName?: string;
  fromEmail?: string;
  content?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: MailchimpCampaignParams
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
    const response = await gateway.call('mailchimp.campaign', {
      action: params.action,
      campaignId: params.campaignId,
      listId: params.listId,
      subject: params.subject,
      fromName: params.fromName,
      fromEmail: params.fromEmail,
      content: params.content,
      limit: params.limit || 10,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Mailchimp campaign action',
    };
  }
}
