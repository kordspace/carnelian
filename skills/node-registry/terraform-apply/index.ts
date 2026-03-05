import type { SkillContext, SkillResult } from '../../types';

interface TerraformApplyParams {
  workingDir: string;
  autoApprove?: boolean;
  vars?: Record<string, string>;
}

export async function execute(
  context: SkillContext,
  params: TerraformApplyParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.workingDir) {
    return {
      success: false,
      error: 'workingDir is required',
    };
  }

  try {
    const response = await gateway.call('terraform.apply', {
      workingDir: params.workingDir,
      autoApprove: params.autoApprove || false,
      vars: params.vars || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to apply Terraform',
    };
  }
}
