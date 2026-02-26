import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface MailchimpAddSubscriberParams {
  listId: string;
  email: string;
  firstName?: string;
  lastName?: string;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: MailchimpAddSubscriberParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.listId || !params.email) {
    return {
      success: false,
      error: 'listId and email are required',
    };
  }

  try {
    const response = await gateway.call('mailchimp.addSubscriber', {
      listId: params.listId,
      email: params.email,
      firstName: params.firstName,
      lastName: params.lastName,
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to add Mailchimp subscriber',
    };
  }
}
