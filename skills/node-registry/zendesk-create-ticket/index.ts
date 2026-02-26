import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ZendeskCreateTicketParams {
  subject: string;
  description: string;
  priority?: 'low' | 'normal' | 'high' | 'urgent';
  requesterEmail?: string;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: ZendeskCreateTicketParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.subject || !params.description) {
    return {
      success: false,
      error: 'subject and description are required',
    };
  }

  try {
    const response = await gateway.call('zendesk.createTicket', {
      subject: params.subject,
      description: params.description,
      priority: params.priority || 'normal',
      requesterEmail: params.requesterEmail,
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Zendesk ticket',
    };
  }
}
