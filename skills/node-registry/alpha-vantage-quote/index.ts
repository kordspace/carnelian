import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AlphaVantageQuoteParams {
  symbol: string;
  function?: string;
  interval?: string;
  outputSize?: string;
}

export async function execute(
  context: SkillContext,
  params: AlphaVantageQuoteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.symbol) {
    return {
      success: false,
      error: 'symbol is required',
    };
  }

  try {
    const response = await gateway.call('alphavantage.quote', {
      symbol: params.symbol,
      function: params.function || 'TIME_SERIES_INTRADAY',
      interval: params.interval || '5min',
      outputSize: params.outputSize || 'compact',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch Alpha Vantage quote',
    };
  }
}
