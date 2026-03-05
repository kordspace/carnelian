import type { SkillContext, SkillResult } from '../../types';

interface AzureCLIParams {
  command: string;
  resourceGroup?: string;
  subscription?: string;
  output?: 'json' | 'table' | 'yaml';
}

export async function execute(
  context: SkillContext,
  params: AzureCLIParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.command) {
    return {
      success: false,
      error: 'command is required',
    };
  }

  try {
    const response = await gateway.call('azure.cli', {
      command: params.command,
      resourceGroup: params.resourceGroup,
      subscription: params.subscription,
      output: params.output || 'json',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Azure CLI command',
    };
  }
}
