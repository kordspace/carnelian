import type { SkillContext, SkillResult } from '../../types';

interface KubernetesDeployParams {
  namespace: string;
  manifest: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: KubernetesDeployParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.namespace || !params.manifest) {
    return {
      success: false,
      error: 'namespace and manifest are required',
    };
  }

  try {
    const response = await gateway.call('kubernetes.deploy', {
      namespace: params.namespace,
      manifest: params.manifest,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to deploy to Kubernetes',
    };
  }
}
