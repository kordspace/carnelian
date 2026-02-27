import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, secretName, version = 'latest' } = context.parameters;

    if (!projectId || !secretName) {
      return { success: false, error: 'projectId and secretName are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/secretmanager/access`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, secretName, version }),
    });

    if (!response.ok) {
      return { success: false, error: `Secret Manager API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to access secret' };
  }
}
