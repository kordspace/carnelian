import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { projectId, zone, instanceName, action = 'start', machineType, image } = context.parameters;

    if (!projectId || !zone || !instanceName) {
      return { success: false, error: 'projectId, zone, and instanceName are required' };
    }

    const response = await fetch(`${context.gateway_url}/gcp/compute/vm`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectId, zone, instanceName, action, machineType, image }),
    });

    if (!response.ok) {
      return { success: false, error: `Compute Engine API error: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to manage Compute Engine VM' };
  }
}
