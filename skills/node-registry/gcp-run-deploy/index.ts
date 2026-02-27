import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, serviceName, region = 'us-central1', image, port = 8080 } = context.parameters;

    if (!projectId || !serviceName || !image) {
      return { success: false, error: 'projectId, serviceName, and image are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/run/deploy`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, serviceName, region, image, port }),
    });

    if (!response.ok) {
      return { success: false, error: `Cloud Run API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to deploy to Cloud Run' };
  }
}
