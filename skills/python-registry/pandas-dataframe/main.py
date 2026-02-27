import pandas as pd
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Execute DataFrame operations using pandas.
    
    Supported operations:
    - filter: Filter rows based on conditions
    - group: Group by columns and aggregate
    - merge: Merge/join DataFrames
    - aggregate: Aggregate operations
    - sort: Sort by columns
    - pivot: Pivot tables
    """
    try:
        params = context.get('parameters', {})
        operation = params.get('operation', 'filter')
        data = params.get('data')
        
        if not data:
            return {'success': False, 'error': 'data parameter is required'}
        
        # Convert data to DataFrame
        df = pd.DataFrame(data)
        
        if operation == 'filter':
            query = params.get('query')
            if query:
                result_df = df.query(query)
            else:
                return {'success': False, 'error': 'query parameter required for filter operation'}
                
        elif operation == 'group':
            group_by = params.get('group_by', [])
            agg_func = params.get('agg_func', 'mean')
            if not group_by:
                return {'success': False, 'error': 'group_by parameter required'}
            result_df = df.groupby(group_by).agg(agg_func)
            
        elif operation == 'merge':
            other_data = params.get('other_data')
            on = params.get('on')
            how = params.get('how', 'inner')
            if not other_data or not on:
                return {'success': False, 'error': 'other_data and on parameters required for merge'}
            other_df = pd.DataFrame(other_data)
            result_df = df.merge(other_df, on=on, how=how)
            
        elif operation == 'aggregate':
            agg_dict = params.get('agg_dict', {})
            if not agg_dict:
                return {'success': False, 'error': 'agg_dict parameter required'}
            result_df = df.agg(agg_dict)
            
        elif operation == 'sort':
            by = params.get('by', [])
            ascending = params.get('ascending', True)
            if not by:
                return {'success': False, 'error': 'by parameter required for sort'}
            result_df = df.sort_values(by=by, ascending=ascending)
            
        elif operation == 'pivot':
            index = params.get('index')
            columns = params.get('columns')
            values = params.get('values')
            if not all([index, columns, values]):
                return {'success': False, 'error': 'index, columns, and values required for pivot'}
            result_df = df.pivot_table(index=index, columns=columns, values=values)
            
        else:
            return {'success': False, 'error': f'Unknown operation: {operation}'}
        
        # Convert result to dict
        result = result_df.to_dict(orient='records')
        
        return {
            'success': True,
            'data': {
                'result': result,
                'shape': result_df.shape,
                'columns': list(result_df.columns)
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
