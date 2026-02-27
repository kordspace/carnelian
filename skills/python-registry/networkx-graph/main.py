import networkx as nx
from typing import Dict, Any

def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """
    Graph analysis and algorithms using NetworkX.
    
    Supported operations:
    - create: Create a graph from edges
    - centrality: Calculate centrality metrics
    - shortest_path: Find shortest paths
    - clustering: Calculate clustering coefficients
    - components: Find connected components
    - pagerank: Calculate PageRank
    """
    try:
        params = context.get('parameters', {})
        operation = params.get('operation', 'create')
        
        if operation == 'create':
            edges = params.get('edges', [])
            directed = params.get('directed', False)
            weighted = params.get('weighted', False)
            
            G = nx.DiGraph() if directed else nx.Graph()
            
            if weighted:
                G.add_weighted_edges_from(edges)
            else:
                G.add_edges_from(edges)
            
            return {
                'success': True,
                'data': {
                    'nodes': list(G.nodes()),
                    'edges': list(G.edges()),
                    'n_nodes': G.number_of_nodes(),
                    'n_edges': G.number_of_edges(),
                    'directed': directed
                }
            }
            
        elif operation == 'centrality':
            edges = params.get('edges', [])
            metric = params.get('metric', 'degree')
            
            G = nx.Graph()
            G.add_edges_from(edges)
            
            if metric == 'degree':
                centrality = nx.degree_centrality(G)
            elif metric == 'betweenness':
                centrality = nx.betweenness_centrality(G)
            elif metric == 'closeness':
                centrality = nx.closeness_centrality(G)
            elif metric == 'eigenvector':
                centrality = nx.eigenvector_centrality(G, max_iter=1000)
            else:
                return {'success': False, 'error': f'Unknown centrality metric: {metric}'}
            
            return {
                'success': True,
                'data': {
                    'centrality': centrality,
                    'metric': metric,
                    'top_nodes': sorted(centrality.items(), key=lambda x: x[1], reverse=True)[:10]
                }
            }
            
        elif operation == 'shortest_path':
            edges = params.get('edges', [])
            source = params.get('source')
            target = params.get('target')
            
            if source is None or target is None:
                return {'success': False, 'error': 'source and target required for shortest_path'}
            
            G = nx.Graph()
            G.add_edges_from(edges)
            
            try:
                path = nx.shortest_path(G, source=source, target=target)
                length = nx.shortest_path_length(G, source=source, target=target)
                
                return {
                    'success': True,
                    'data': {
                        'path': path,
                        'length': length
                    }
                }
            except nx.NetworkXNoPath:
                return {'success': False, 'error': 'No path exists between source and target'}
                
        elif operation == 'clustering':
            edges = params.get('edges', [])
            
            G = nx.Graph()
            G.add_edges_from(edges)
            
            clustering_coeffs = nx.clustering(G)
            avg_clustering = nx.average_clustering(G)
            
            return {
                'success': True,
                'data': {
                    'clustering_coefficients': clustering_coeffs,
                    'average_clustering': float(avg_clustering)
                }
            }
            
        elif operation == 'components':
            edges = params.get('edges', [])
            directed = params.get('directed', False)
            
            G = nx.DiGraph() if directed else nx.Graph()
            G.add_edges_from(edges)
            
            if directed:
                components = list(nx.strongly_connected_components(G))
            else:
                components = list(nx.connected_components(G))
            
            return {
                'success': True,
                'data': {
                    'n_components': len(components),
                    'components': [list(comp) for comp in components],
                    'largest_component_size': len(max(components, key=len)) if components else 0
                }
            }
            
        elif operation == 'pagerank':
            edges = params.get('edges', [])
            alpha = params.get('alpha', 0.85)
            
            G = nx.DiGraph()
            G.add_edges_from(edges)
            
            pagerank = nx.pagerank(G, alpha=alpha)
            
            return {
                'success': True,
                'data': {
                    'pagerank': pagerank,
                    'top_nodes': sorted(pagerank.items(), key=lambda x: x[1], reverse=True)[:10]
                }
            }
            
        else:
            return {'success': False, 'error': f'Unknown operation: {operation}'}
        
    except Exception as e:
        return {'success': False, 'error': str(e)}
