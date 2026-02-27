import plotly.graph_objects as go
import plotly.express as px
import pandas as pd
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Create interactive visualizations using Plotly.
    
    Supported plot types:
    - line: Interactive line plot
    - scatter: Interactive scatter plot
    - bar: Interactive bar chart
    - histogram: Interactive histogram
    - box: Interactive box plot
    - heatmap: Interactive heatmap
    - 3d_scatter: 3D scatter plot
    """
    try:
        params = context.get('parameters', {})
        plot_type = params.get('plot_type', 'line')
        data = params.get('data')
        
        if data is None:
            return {'success': False, 'error': 'data is required'}
        
        # Convert to DataFrame
        df = pd.DataFrame(data)
        
        # Create plot based on type
        if plot_type == 'line':
            x = params.get('x')
            y = params.get('y')
            if not x or not y:
                return {'success': False, 'error': 'x and y required for line plot'}
            fig = px.line(df, x=x, y=y, title=params.get('title', ''))
            
        elif plot_type == 'scatter':
            x = params.get('x')
            y = params.get('y')
            if not x or not y:
                return {'success': False, 'error': 'x and y required for scatter plot'}
            fig = px.scatter(df, x=x, y=y, color=params.get('color'), 
                           size=params.get('size'), title=params.get('title', ''))
            
        elif plot_type == 'bar':
            x = params.get('x')
            y = params.get('y')
            if not x or not y:
                return {'success': False, 'error': 'x and y required for bar chart'}
            fig = px.bar(df, x=x, y=y, color=params.get('color'), 
                        title=params.get('title', ''))
            
        elif plot_type == 'histogram':
            x = params.get('x')
            if not x:
                return {'success': False, 'error': 'x required for histogram'}
            fig = px.histogram(df, x=x, nbins=params.get('nbins', 10), 
                             title=params.get('title', ''))
            
        elif plot_type == 'box':
            y = params.get('y')
            if not y:
                return {'success': False, 'error': 'y required for box plot'}
            fig = px.box(df, x=params.get('x'), y=y, color=params.get('color'),
                        title=params.get('title', ''))
            
        elif plot_type == 'heatmap':
            z = params.get('z')
            if not z:
                # Use correlation matrix
                z = df.corr().values.tolist()
                x = y = df.columns.tolist()
            else:
                x = params.get('x', list(range(len(z[0]))))
                y = params.get('y', list(range(len(z))))
            fig = go.Figure(data=go.Heatmap(z=z, x=x, y=y))
            fig.update_layout(title=params.get('title', ''))
            
        elif plot_type == '3d_scatter':
            x = params.get('x')
            y = params.get('y')
            z = params.get('z')
            if not x or not y or not z:
                return {'success': False, 'error': 'x, y, and z required for 3D scatter'}
            fig = px.scatter_3d(df, x=x, y=y, z=z, color=params.get('color'),
                              title=params.get('title', ''))
            
        else:
            return {'success': False, 'error': f'Unknown plot type: {plot_type}'}
        
        # Convert to HTML
        html = fig.to_html(include_plotlyjs='cdn')
        
        # Also get JSON for programmatic use
        json_data = fig.to_json()
        
        return {
            'success': True,
            'data': {
                'html': html,
                'json': json_data,
                'plot_type': plot_type
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
