import pandas as pd
import csv
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Advanced CSV file processing and transformation.
    
    Supported operations:
    - read: Read CSV file
    - write: Write CSV file
    - merge: Merge multiple CSV files
    - clean: Clean CSV data (remove duplicates, handle missing values)
    - transform: Transform columns
    """
    try:
        params = context.get('parameters', {})
        operation = params.get('operation', 'read')
        
        if operation == 'read':
            path = params.get('path')
            if not path:
                return {'success': False, 'error': 'path is required'}
            
            df = pd.read_csv(path, **params.get('read_options', {}))
            
            return {
                'success': True,
                'data': {
                    'result': df.to_dict(orient='records'),
                    'shape': df.shape,
                    'columns': df.columns.tolist()
                }
            }
            
        elif operation == 'write':
            data = params.get('data')
            path = params.get('path', '/tmp/output.csv')
            
            if data is None:
                return {'success': False, 'error': 'data is required'}
            
            df = pd.DataFrame(data)
            df.to_csv(path, index=False, **params.get('write_options', {}))
            
            return {
                'success': True,
                'data': {
                    'path': path,
                    'rows_written': len(df)
                }
            }
            
        elif operation == 'merge':
            files = params.get('files', [])
            if not files:
                return {'success': False, 'error': 'files list is required'}
            
            dfs = [pd.read_csv(f) for f in files]
            merged = pd.concat(dfs, ignore_index=True)
            
            return {
                'success': True,
                'data': {
                    'result': merged.to_dict(orient='records'),
                    'shape': merged.shape,
                    'files_merged': len(files)
                }
            }
            
        elif operation == 'clean':
            data = params.get('data')
            if data is None:
                return {'success': False, 'error': 'data is required'}
            
            df = pd.DataFrame(data)
            
            # Remove duplicates
            if params.get('remove_duplicates', True):
                df = df.drop_duplicates()
            
            # Handle missing values
            missing_strategy = params.get('missing_strategy', 'drop')
            if missing_strategy == 'drop':
                df = df.dropna()
            elif missing_strategy == 'fill':
                fill_value = params.get('fill_value', 0)
                df = df.fillna(fill_value)
            
            return {
                'success': True,
                'data': {
                    'result': df.to_dict(orient='records'),
                    'shape': df.shape,
                    'rows_removed': len(pd.DataFrame(data)) - len(df)
                }
            }
            
        elif operation == 'transform':
            data = params.get('data')
            transformations = params.get('transformations', {})
            
            if data is None:
                return {'success': False, 'error': 'data is required'}
            
            df = pd.DataFrame(data)
            
            # Apply transformations
            for col, transform in transformations.items():
                if transform == 'uppercase':
                    df[col] = df[col].str.upper()
                elif transform == 'lowercase':
                    df[col] = df[col].str.lower()
                elif transform == 'strip':
                    df[col] = df[col].str.strip()
                elif transform == 'numeric':
                    df[col] = pd.to_numeric(df[col], errors='coerce')
            
            return {
                'success': True,
                'data': {
                    'result': df.to_dict(orient='records'),
                    'shape': df.shape,
                    'transformations_applied': len(transformations)
                }
            }
            
        else:
            return {'success': False, 'error': f'Unknown operation: {operation}'}
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
