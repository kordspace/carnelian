from statsmodels.tsa.seasonal import seasonal_decompose
from statsmodels.tsa.arima.model import ARIMA
from statsmodels.tsa.stattools import adfuller
import pandas as pd
import numpy as np
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Time series analysis and forecasting.
    
    Supported operations:
    - decompose: Seasonal decomposition
    - stationarity: Test for stationarity (ADF test)
    - arima: ARIMA forecasting
    - moving_average: Calculate moving averages
    """
    try:
        params = context.get('parameters', {})
        operation = params.get('operation', 'decompose')
        data = params.get('data')
        
        if data is None:
            return {'success': False, 'error': 'data is required'}
        
        # Convert to Series
        ts = pd.Series(data)
        
        if operation == 'decompose':
            period = params.get('period', 12)
            model = params.get('model', 'additive')
            
            decomposition = seasonal_decompose(ts, model=model, period=period)
            
            return {
                'success': True,
                'data': {
                    'trend': decomposition.trend.dropna().tolist(),
                    'seasonal': decomposition.seasonal.dropna().tolist(),
                    'residual': decomposition.resid.dropna().tolist()
                }
            }
            
        elif operation == 'stationarity':
            result = adfuller(ts.dropna())
            
            return {
                'success': True,
                'data': {
                    'adf_statistic': float(result[0]),
                    'p_value': float(result[1]),
                    'is_stationary': result[1] < 0.05,
                    'critical_values': {k: float(v) for k, v in result[4].items()}
                }
            }
            
        elif operation == 'arima':
            order = params.get('order', (1, 1, 1))
            steps = params.get('steps', 10)
            
            model = ARIMA(ts, order=order)
            fitted = model.fit()
            
            # Forecast
            forecast = fitted.forecast(steps=steps)
            
            return {
                'success': True,
                'data': {
                    'forecast': forecast.tolist(),
                    'aic': float(fitted.aic),
                    'bic': float(fitted.bic),
                    'order': order
                }
            }
            
        elif operation == 'moving_average':
            window = params.get('window', 7)
            
            ma = ts.rolling(window=window).mean()
            
            return {
                'success': True,
                'data': {
                    'moving_average': ma.dropna().tolist(),
                    'window': window
                }
            }
            
        else:
            return {'success': False, 'error': f'Unknown operation: {operation}'}
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
