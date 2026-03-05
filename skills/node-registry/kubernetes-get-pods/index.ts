import type { SkillContext, SkillResult } from '../../types';

interface KubernetesGetPodsParams {
  namespace?: string;
  labelSelector?: string;
}

export async function execute(
  context: SkillContext,
  params: KubernetesGetPodsParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('kubernetes.getPods', {
      namespace: params.namespace || 'default',
      labelSelector: params.labelSelector,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get Kubernetes pods',
    };
  }
}
