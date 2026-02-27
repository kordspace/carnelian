import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, functionName, region = 'us-central1', runtime, entryPoint, sourceArchiveUrl } = context.parameters;

    if (!projectId || !functionName || !runtime || !entryPoint || !sourceArchiveUrl) {
      return { success: false, error: 'projectId, functionName, runtime, entryPoint, and sourceArchiveUrl are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/functions/deploy`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, functionName, region, runtime, entryPoint, sourceArchiveUrl }),
    });

    if (!response.ok) {
      return { success: false, error: `Cloud Functions API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to deploy Cloud Function' };
  }
}
