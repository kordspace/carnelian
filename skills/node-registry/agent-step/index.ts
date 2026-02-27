import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AgentStepParams {
  agentId: string;
  message: string;
  context?: Record<string, unknown>;
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: AgentStepParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.agentId || !params.message) {
    return {
      success: false,
      error: 'agentId and message are required',
    };
  }

  try {
    const response = await gateway.call('agent.step', {
      agentId: params.agentId,
      message: params.message,
      context: params.context || {},
      timeout: params.timeout || 30000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute agent step',
    };
  }
}
