import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { bucketName, fileName, destination } = context.parameters;

    if (!bucketName || !fileName) {
      return { success: false, error: 'bucketName and fileName are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/storage/download`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bucketName, fileName, destination }),
    });

    if (!response.ok) {
      return { success: false, error: `Cloud Storage API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to download from Cloud Storage' };
  }
}
