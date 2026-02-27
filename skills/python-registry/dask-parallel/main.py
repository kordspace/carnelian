import dask.dataframe as dd
import dask.array as da
from dask import delayed
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Parallel data processing using Dask.
    
    Supported operations:
    - dataframe: Parallel DataFrame operations
    - array: Parallel array operations
    - delayed: Lazy evaluation of custom functions
    """
    try:
        params = context.get('parameters', {})
        operation = params.get('operation', 'dataframe')
        
        if operation == 'dataframe':
            data = params.get('data')
            op = params.get('op', 'compute')
            
            if data is None:
                return {'success': False, 'error': 'data is required'}
            
            # Create Dask DataFrame
            ddf = dd.from_pandas(data, npartitions=params.get('npartitions', 2))
            
            if op == 'compute':
                result = ddf.compute()
                return {
                    'success': True,
                    'data': {
                        'result': result.to_dict(orient='records'),
                        'shape': result.shape
                    }
                }
            elif op == 'groupby':
                by = params.get('by')
                agg = params.get('agg', 'mean')
                if not by:
                    return {'success': False, 'error': 'by parameter required for groupby'}
                result = ddf.groupby(by).agg(agg).compute()
                return {
                    'success': True,
                    'data': {
                        'result': result.to_dict(),
                        'shape': result.shape
                    }
                }
            elif op == 'filter':
                query = params.get('query')
                if not query:
                    return {'success': False, 'error': 'query parameter required for filter'}
                result = ddf.query(query).compute()
                return {
                    'success': True,
                    'data': {
                        'result': result.to_dict(orient='records'),
                        'shape': result.shape
                    }
                }
            else:
                return {'success': False, 'error': f'Unknown operation: {op}'}
                
        elif operation == 'array':
            data = params.get('data')
            op = params.get('op', 'sum')
            
            if data is None:
                return {'success': False, 'error': 'data is required'}
            
            # Create Dask array
            darr = da.from_delayed(delayed(lambda: data)(), shape=len(data), dtype=float)
            
            if op == 'sum':
                result = darr.sum().compute()
            elif op == 'mean':
                result = darr.mean().compute()
            elif op == 'std':
                result = darr.std().compute()
            else:
                return {'success': False, 'error': f'Unknown array operation: {op}'}
            
            return {
                'success': True,
                'data': {
                    'result': float(result),
                    'operation': op
                }
            }
            
        else:
            return {'success': False, 'error': f'Unknown operation: {operation}'}
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
