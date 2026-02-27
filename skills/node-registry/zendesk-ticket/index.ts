import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ZendeskTicketParams {
  action: 'create' | 'update' | 'get' | 'list' | 'add_comment';
  ticketId?: string;
  subject?: string;
  description?: string;
  priority?: 'low' | 'normal' | 'high' | 'urgent';
  status?: string;
  comment?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: ZendeskTicketParams
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
    const response = await gateway.call('zendesk.ticket', {
      action: params.action,
      ticketId: params.ticketId,
      subject: params.subject,
      description: params.description,
      priority: params.priority || 'normal',
      status: params.status,
      comment: params.comment,
      limit: params.limit || 25,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Zendesk ticket action',
    };
  }
}
