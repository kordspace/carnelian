import polars as pl
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Fast DataFrame queries using Polars.
    
    Supported operations:
    - filter: Filter rows
    - select: Select columns
    - group_by: Group and aggregate
    - join: Join DataFrames
    - sort: Sort by columns
    """
    try:
        params = context.get('parameters', {})
        operation = params.get('operation', 'filter')
        data = params.get('data')
        
        if data is None:
            return {'success': False, 'error': 'data is required'}
        
        # Convert to Polars DataFrame
        df = pl.DataFrame(data)
        
        if operation == 'filter':
            expr = params.get('expression')
            if not expr:
                return {'success': False, 'error': 'expression required for filter'}
            result_df = df.filter(pl.col(expr['column']).eval(expr['condition']))
            
        elif operation == 'select':
            columns = params.get('columns', [])
            if not columns:
                return {'success': False, 'error': 'columns required for select'}
            result_df = df.select(columns)
            
        elif operation == 'group_by':
            by = params.get('by', [])
            agg = params.get('agg', {})
            if not by or not agg:
                return {'success': False, 'error': 'by and agg required for group_by'}
            result_df = df.group_by(by).agg([pl.col(col).eval(func) for col, func in agg.items()])
            
        elif operation == 'join':
            other_data = params.get('other_data')
            on = params.get('on')
            how = params.get('how', 'inner')
            if not other_data or not on:
                return {'success': False, 'error': 'other_data and on required for join'}
            other_df = pl.DataFrame(other_data)
            result_df = df.join(other_df, on=on, how=how)
            
        elif operation == 'sort':
            by = params.get('by', [])
            descending = params.get('descending', False)
            if not by:
                return {'success': False, 'error': 'by required for sort'}
            result_df = df.sort(by, descending=descending)
            
        else:
            return {'success': False, 'error': f'Unknown operation: {operation}'}
        
        # Convert to dict
        result = result_df.to_dicts()
        
        return {
            'success': True,
            'data': {
                'result': result,
                'shape': result_df.shape,
                'columns': result_df.columns
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
