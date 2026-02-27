import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { bucketName, prefix = '', maxResults = 1000 } = context.parameters;

    if (!bucketName) {
      return { success: false, error: 'bucketName is required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/storage/list`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bucketName, prefix, maxResults }),
    });

    if (!response.ok) {
      return { success: false, error: `Cloud Storage API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to list Cloud Storage objects' };
  }
}
