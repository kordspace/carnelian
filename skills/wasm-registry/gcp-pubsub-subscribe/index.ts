import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, subscriptionName, maxMessages = 10, returnImmediately = false } = context.parameters;

    if (!projectId || !subscriptionName) {
      return { success: false, error: 'projectId and subscriptionName are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/pubsub/subscribe`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, subscriptionName, maxMessages, returnImmediately }),
    });

    if (!response.ok) {
      return { success: false, error: `Pub/Sub API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to subscribe to Pub/Sub' };
  }
}
