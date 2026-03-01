"""Qiskit Quantum RNG Skill

Generates quantum entropy using IBM Quantum backends via Qiskit circuits.
"""

from typing import Dict, Any


def generate_quantum_entropy(n_bits: int, backend_name: str) -> Dict[str, Any]:
    """Generate quantum entropy using IBM Quantum backend.
    
    Args:
        n_bits: Number of random bits to generate
        backend_name: IBM Quantum backend name (e.g., "ibm_brisbane")
    
    Returns:
        Dictionary with entropy_hex, bits, and device
    """
    from qiskit import QuantumCircuit
    from qiskit_ibm_runtime import QiskitRuntimeService
    
    # Initialize service and get backend
    service = QiskitRuntimeService()
    backend = service.backend(backend_name)
    
    # Create quantum circuit with n_bits qubits
    qc = QuantumCircuit(n_bits, n_bits)
    
    # Apply Hadamard gate to each qubit (creates superposition)
    for i in range(n_bits):
        qc.h(i)
    
    # Measure all qubits
    qc.measure(range(n_bits), range(n_bits))
    
    # Run circuit on backend (single shot)
    job = backend.run(qc, shots=1)
    result = job.result()
    
    # Extract measurement outcome
    counts = result.get_counts()
    bitstring = list(counts.keys())[0]  # Get the single measurement result
    
    # Convert bitstring to bytes
    n_bytes = (n_bits + 7) // 8
    byte_array = bytearray(n_bytes)
    
    for i, bit in enumerate(bitstring):
        if bit == '1':
            byte_idx = i // 8
            bit_idx = i % 8
            byte_array[byte_idx] |= (1 << bit_idx)
    
    entropy_hex = byte_array.hex()
    
    return {
        "entropy_hex": entropy_hex,
        "bits": bitstring,
        "device": backend_name,
    }


def execute(context: Dict[str, Any]) -> Dict[str, Any]:
    """Skill entrypoint for Carnelian worker execution.
    
    Args:
        context: Execution context with parameters
    
    Returns:
        Success/failure result with data or error
    """
    try:
        params = context.get("parameters", {})
        n_bits = params.get("n_bits", 256)
        backend_name = params.get("backend_name", "ibm_brisbane")
        
        result = generate_quantum_entropy(n_bits, backend_name)
        
        return {
            "success": True,
            "data": result,
        }
    except Exception as e:
        return {
            "success": False,
            "error": str(e),
        }
