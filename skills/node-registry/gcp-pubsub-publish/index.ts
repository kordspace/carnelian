import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, topicName, message, attributes = {} } = context.parameters;

    if (!projectId || !topicName || !message) {
      return { success: false, error: 'projectId, topicName, and message are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/pubsub/publish`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, topicName, message, attributes }),
    });

    if (!response.ok) {
      return { success: false, error: `Pub/Sub API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to publish to Pub/Sub' };
  }
}
