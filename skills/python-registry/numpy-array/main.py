import numpy as np
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Execute NumPy array operations.
    
    Supported operations:
    - matmul: Matrix multiplication
    - dot: Dot product
    - transpose: Matrix transpose
    - inverse: Matrix inverse
    - eigenvalues: Compute eigenvalues
    - svd: Singular value decomposition
    - stats: Statistical operations (mean, std, var, etc.)
    - reshape: Reshape array
    """
    try:
        params = context.get('parameters', {})
        operation = params.get('operation', 'matmul')
        data = params.get('data')
        
        if data is None:
            return {'success': False, 'error': 'data parameter is required'}
        
        # Convert to numpy array
        arr = np.array(data)
        
        if operation == 'matmul':
            other_data = params.get('other_data')
            if other_data is None:
                return {'success': False, 'error': 'other_data required for matmul'}
            other_arr = np.array(other_data)
            result = np.matmul(arr, other_arr)
            
        elif operation == 'dot':
            other_data = params.get('other_data')
            if other_data is None:
                return {'success': False, 'error': 'other_data required for dot'}
            other_arr = np.array(other_data)
            result = np.dot(arr, other_arr)
            
        elif operation == 'transpose':
            result = arr.T
            
        elif operation == 'inverse':
            result = np.linalg.inv(arr)
            
        elif operation == 'eigenvalues':
            eigenvalues, eigenvectors = np.linalg.eig(arr)
            return {
                'success': True,
                'data': {
                    'eigenvalues': eigenvalues.tolist(),
                    'eigenvectors': eigenvectors.tolist()
                }
            }
            
        elif operation == 'svd':
            U, S, Vh = np.linalg.svd(arr)
            return {
                'success': True,
                'data': {
                    'U': U.tolist(),
                    'S': S.tolist(),
                    'Vh': Vh.tolist()
                }
            }
            
        elif operation == 'stats':
            return {
                'success': True,
                'data': {
                    'mean': float(np.mean(arr)),
                    'std': float(np.std(arr)),
                    'var': float(np.var(arr)),
                    'min': float(np.min(arr)),
                    'max': float(np.max(arr)),
                    'median': float(np.median(arr))
                }
            }
            
        elif operation == 'reshape':
            shape = params.get('shape')
            if not shape:
                return {'success': False, 'error': 'shape parameter required for reshape'}
            result = arr.reshape(shape)
            
        else:
            return {'success': False, 'error': f'Unknown operation: {operation}'}
        
        return {
            'success': True,
            'data': {
                'result': result.tolist(),
                'shape': list(result.shape),
                'dtype': str(result.dtype)
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
