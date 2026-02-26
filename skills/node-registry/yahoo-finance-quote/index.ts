import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface YahooFinanceQuoteParams {
  symbols: string[];
}

export async function execute(
  context: SkillContext,
  params: YahooFinanceQuoteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.symbols || params.symbols.length === 0) {
    return {
      success: false,
      error: 'symbols is required',
    };
  }

  try {
    const response = await gateway.call('yahooFinance.getQuote', {
      symbols: params.symbols,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get Yahoo Finance quote',
    };
  }
}
