from scipy import optimize
import numpy as np
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Optimization algorithms using SciPy.
    
    Supported methods:
    - minimize: Minimize a scalar function
    - curve_fit: Fit a curve to data
    - root: Find roots of equations
    - linear_programming: Linear programming
    """
    try:
        params = context.get('parameters', {})
        method = params.get('method', 'minimize')
        
        if method == 'minimize':
            # Function to minimize (provided as string or coefficients)
            x0 = params.get('x0', [0])
            bounds = params.get('bounds')
            algorithm = params.get('algorithm', 'BFGS')
            
            # Simple quadratic function for demonstration
            def objective(x):
                coeffs = params.get('coefficients', [1, 0, 0])  # ax^2 + bx + c
                return sum(c * (x[0] ** i) for i, c in enumerate(coeffs))
            
            result = optimize.minimize(objective, x0, method=algorithm, bounds=bounds)
            
            return {
                'success': True,
                'data': {
                    'x': result.x.tolist(),
                    'fun': float(result.fun),
                    'success': bool(result.success),
                    'message': result.message,
                    'nit': int(result.nit) if hasattr(result, 'nit') else None
                }
            }
            
        elif method == 'curve_fit':
            xdata = params.get('xdata')
            ydata = params.get('ydata')
            curve_type = params.get('curve_type', 'linear')
            
            if xdata is None or ydata is None:
                return {'success': False, 'error': 'xdata and ydata required for curve_fit'}
            
            # Define curve functions
            if curve_type == 'linear':
                def func(x, a, b):
                    return a * x + b
                p0 = [1, 0]
            elif curve_type == 'exponential':
                def func(x, a, b, c):
                    return a * np.exp(b * x) + c
                p0 = [1, 1, 0]
            elif curve_type == 'polynomial':
                degree = params.get('degree', 2)
                def func(x, *coeffs):
                    return sum(c * (x ** i) for i, c in enumerate(coeffs))
                p0 = [1] * (degree + 1)
            else:
                return {'success': False, 'error': f'Unknown curve type: {curve_type}'}
            
            popt, pcov = optimize.curve_fit(func, xdata, ydata, p0=p0)
            
            return {
                'success': True,
                'data': {
                    'parameters': popt.tolist(),
                    'covariance': pcov.tolist(),
                    'curve_type': curve_type
                }
            }
            
        elif method == 'root':
            x0 = params.get('x0', [0])
            
            # Simple function for root finding
            def func(x):
                coeffs = params.get('coefficients', [1, 0, -1])  # ax^2 + bx + c
                return sum(c * (x[0] ** i) for i, c in enumerate(coeffs))
            
            result = optimize.root(func, x0)
            
            return {
                'success': True,
                'data': {
                    'x': result.x.tolist(),
                    'success': bool(result.success),
                    'message': result.message
                }
            }
            
        elif method == 'linear_programming':
            c = params.get('c')  # Coefficients of linear objective function
            A_ub = params.get('A_ub')  # Inequality constraint matrix
            b_ub = params.get('b_ub')  # Inequality constraint bounds
            
            if c is None:
                return {'success': False, 'error': 'c (objective coefficients) required'}
            
            result = optimize.linprog(c, A_ub=A_ub, b_ub=b_ub, method='highs')
            
            return {
                'success': True,
                'data': {
                    'x': result.x.tolist() if result.x is not None else None,
                    'fun': float(result.fun) if result.fun is not None else None,
                    'success': bool(result.success),
                    'message': result.message
                }
            }
            
        else:
            return {'success': False, 'error': f'Unknown method: {method}'}
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
