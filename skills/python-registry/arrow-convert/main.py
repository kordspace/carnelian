import pyarrow as pa
import pyarrow.parquet as pq
import pandas as pd
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Apache Arrow data format conversions.
    
    Supported operations:
    - to_arrow: Convert data to Arrow Table
    - from_arrow: Convert Arrow Table to dict
    - to_parquet: Write to Parquet format
    - from_parquet: Read from Parquet format
    """
    try:
        params = context.get('parameters', {})
        operation = params.get('operation', 'to_arrow')
        
        if operation == 'to_arrow':
            data = params.get('data')
            if data is None:
                return {'success': False, 'error': 'data is required'}
            
            # Convert to Arrow Table
            df = pd.DataFrame(data)
            table = pa.Table.from_pandas(df)
            
            return {
                'success': True,
                'data': {
                    'schema': str(table.schema),
                    'num_rows': table.num_rows,
                    'num_columns': table.num_columns,
                    'column_names': table.column_names
                }
            }
            
        elif operation == 'from_arrow':
            # This would require serialized Arrow data
            return {'success': False, 'error': 'from_arrow not yet implemented'}
            
        elif operation == 'to_parquet':
            data = params.get('data')
            path = params.get('path', '/tmp/output.parquet')
            
            if data is None:
                return {'success': False, 'error': 'data is required'}
            
            df = pd.DataFrame(data)
            table = pa.Table.from_pandas(df)
            pq.write_table(table, path)
            
            return {
                'success': True,
                'data': {
                    'path': path,
                    'num_rows': table.num_rows,
                    'num_columns': table.num_columns
                }
            }
            
        elif operation == 'from_parquet':
            path = params.get('path')
            if not path:
                return {'success': False, 'error': 'path is required'}
            
            table = pq.read_table(path)
            df = table.to_pandas()
            
            return {
                'success': True,
                'data': {
                    'result': df.to_dict(orient='records'),
                    'num_rows': len(df),
                    'num_columns': len(df.columns)
                }
            }
            
        else:
            return {'success': False, 'error': f'Unknown operation: {operation}'}
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
