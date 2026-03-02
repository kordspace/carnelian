import type { SkillContext, SkillResult } from '../../types';

interface FreshdeskCreateTicketParams {
  subject: string;
  description: string;
  email: string;
  priority?: number;
  status?: number;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: FreshdeskCreateTicketParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.subject || !params.description || !params.email) {
    return {
      success: false,
      error: 'subject, description, and email are required',
    };
  }

  try {
    const response = await gateway.call('freshdesk.createTicket', {
      subject: params.subject,
      description: params.description,
      email: params.email,
      priority: params.priority || 1,
      status: params.status || 2,
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Freshdesk ticket',
    };
  }
}
