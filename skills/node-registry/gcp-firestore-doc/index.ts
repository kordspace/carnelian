import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, collection, documentId, action = 'get', data } = context.parameters;

    if (!projectId || !collection) {
      return { success: false, error: 'projectId and collection are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/firestore/document`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, collection, documentId, action, data }),
    });

    if (!response.ok) {
      return { success: false, error: `Firestore API error: ${response.statusText}` };
    }

    const result = await response.json();
    return { success: true, data: result };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute Firestore operation' };
  }
}
