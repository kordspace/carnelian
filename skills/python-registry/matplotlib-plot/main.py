import matplotlib.pyplot as plt
import matplotlib
matplotlib.use('Agg')  # Non-interactive backend
import base64
from io import BytesIO
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Create static plots using matplotlib.
    
    Supported plot types:
    - line: Line plot
    - bar: Bar chart
    - scatter: Scatter plot
    - histogram: Histogram
    - pie: Pie chart
    - box: Box plot
    """
    try:
        params = context.get('parameters', {})
        plot_type = params.get('plot_type', 'line')
        x_data = params.get('x_data')
        y_data = params.get('y_data')
        title = params.get('title', '')
        xlabel = params.get('xlabel', '')
        ylabel = params.get('ylabel', '')
        figsize = params.get('figsize', (10, 6))
        
        # Create figure
        fig, ax = plt.subplots(figsize=figsize)
        
        if plot_type == 'line':
            if x_data is None or y_data is None:
                return {'success': False, 'error': 'x_data and y_data required for line plot'}
            ax.plot(x_data, y_data, **params.get('plot_kwargs', {}))
            
        elif plot_type == 'bar':
            if x_data is None or y_data is None:
                return {'success': False, 'error': 'x_data and y_data required for bar plot'}
            ax.bar(x_data, y_data, **params.get('plot_kwargs', {}))
            
        elif plot_type == 'scatter':
            if x_data is None or y_data is None:
                return {'success': False, 'error': 'x_data and y_data required for scatter plot'}
            ax.scatter(x_data, y_data, **params.get('plot_kwargs', {}))
            
        elif plot_type == 'histogram':
            if x_data is None:
                return {'success': False, 'error': 'x_data required for histogram'}
            bins = params.get('bins', 10)
            ax.hist(x_data, bins=bins, **params.get('plot_kwargs', {}))
            
        elif plot_type == 'pie':
            if y_data is None:
                return {'success': False, 'error': 'y_data required for pie chart'}
            labels = params.get('labels', None)
            ax.pie(y_data, labels=labels, **params.get('plot_kwargs', {}))
            
        elif plot_type == 'box':
            if y_data is None:
                return {'success': False, 'error': 'y_data required for box plot'}
            ax.boxplot(y_data, **params.get('plot_kwargs', {}))
            
        else:
            return {'success': False, 'error': f'Unknown plot type: {plot_type}'}
        
        # Set labels and title
        if title:
            ax.set_title(title)
        if xlabel:
            ax.set_xlabel(xlabel)
        if ylabel:
            ax.set_ylabel(ylabel)
        
        # Add grid if requested
        if params.get('grid', False):
            ax.grid(True)
        
        # Save to base64
        buffer = BytesIO()
        plt.savefig(buffer, format='png', bbox_inches='tight', dpi=params.get('dpi', 100))
        buffer.seek(0)
        image_b64 = base64.b64encode(buffer.read()).decode('utf-8')
        plt.close(fig)
        
        return {
            'success': True,
            'data': {
                'image_b64': image_b64,
                'plot_type': plot_type,
                'format': 'png'
            }
        }
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
