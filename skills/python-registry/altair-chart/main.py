import altair as alt
import pandas as pd
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Create declarative visualizations using Altair.
    
    Supported chart types:
    - line: Line chart
    - bar: Bar chart
    - scatter: Scatter plot
    - area: Area chart
    - histogram: Histogram
    """
    try:
        params = context.get('parameters', {})
        chart_type = params.get('chart_type', 'line')
        data = params.get('data')
        
        if data is None:
            return {'success': False, 'error': 'data is required'}
        
        # Convert to DataFrame
        df = pd.DataFrame(data)
        
        # Create chart based on type
        if chart_type == 'line':
            x = params.get('x')
            y = params.get('y')
            if not x or not y:
                return {'success': False, 'error': 'x and y required for line chart'}
            chart = alt.Chart(df).mark_line().encode(
                x=x,
                y=y,
                color=params.get('color') if params.get('color') else alt.Undefined
            )
            
        elif chart_type == 'bar':
            x = params.get('x')
            y = params.get('y')
            if not x or not y:
                return {'success': False, 'error': 'x and y required for bar chart'}
            chart = alt.Chart(df).mark_bar().encode(
                x=x,
                y=y,
                color=params.get('color') if params.get('color') else alt.Undefined
            )
            
        elif chart_type == 'scatter':
            x = params.get('x')
            y = params.get('y')
            if not x or not y:
                return {'success': False, 'error': 'x and y required for scatter plot'}
            chart = alt.Chart(df).mark_circle().encode(
                x=x,
                y=y,
                color=params.get('color') if params.get('color') else alt.Undefined,
                size=params.get('size') if params.get('size') else alt.Undefined
            )
            
        elif chart_type == 'area':
            x = params.get('x')
            y = params.get('y')
            if not x or not y:
                return {'success': False, 'error': 'x and y required for area chart'}
            chart = alt.Chart(df).mark_area().encode(
                x=x,
                y=y,
                color=params.get('color') if params.get('color') else alt.Undefined
            )
            
        elif chart_type == 'histogram':
            x = params.get('x')
            if not x:
                return {'success': False, 'error': 'x required for histogram'}
            chart = alt.Chart(df).mark_bar().encode(
                x=alt.X(x, bin=True),
                y='count()'
            )
            
        else:
            return {'success': False, 'error': f'Unknown chart type: {chart_type}'}
        
        # Add title if provided
        if params.get('title'):
            chart = chart.properties(title=params['title'])
        
        # Convert to JSON and HTML
        chart_json = chart.to_json()
        chart_html = chart.to_html()
        
        return {
            'success': True,
            'data': {
                'json': chart_json,
                'html': chart_html,
                'chart_type': chart_type
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
