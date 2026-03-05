import type { SkillContext, SkillResult } from '../../types';

interface AWSLambdaInvokeParams {
  functionName: string;
  payload?: any;
  invocationType?: 'RequestResponse' | 'Event' | 'DryRun';
}

export async function execute(
  context: SkillContext,
  params: AWSLambdaInvokeParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.functionName) {
    return {
      success: false,
      error: 'functionName is required',
    };
  }

  try {
    const response = await gateway.call('aws.lambda.invoke', {
      functionName: params.functionName,
      payload: params.payload,
      invocationType: params.invocationType || 'RequestResponse',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to invoke Lambda function',
    };
  }
}
