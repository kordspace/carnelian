"""Quantinuum H2 Quantum RNG Skill

Generates quantum entropy using Quantinuum H-series devices via pytket circuits.
"""

from typing import Dict, Any


def generate_quantum_entropy(n_bits: int, device: str) -> Dict[str, Any]:
    """Generate quantum entropy using Quantinuum H-series device.
    
    Args:
        n_bits: Number of random bits to generate
        device: Quantinuum device name (e.g., "H1-1E" for emulator, "H2-1" for hardware)
    
    Returns:
        Dictionary with entropy_hex, bits, device, and n_bits
    """
    from pytket.extensions.quantinuum import QuantinuumBackend
    from pytket import Circuit
    
    # Create quantum circuit with n_bits qubits
    circuit = Circuit(n_bits)
    
    # Apply Hadamard gate to each qubit (creates superposition)
    for i in range(n_bits):
        circuit.H(i)
    
    # Measure all qubits
    circuit.measure_all()
    
    # Get backend and submit job
    backend = QuantinuumBackend(device_name=device)
    backend.login()
    
    # Compile and run circuit
    compiled_circuit = backend.get_compiled_circuit(circuit)
    handle = backend.process_circuit(compiled_circuit, n_shots=1)
    result = backend.get_result(handle)
    
    # Extract measurement outcome (single shot)
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
        "device": device,
        "n_bits": n_bits,
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
        device = params.get("device", "H1-1E")
        
        result = generate_quantum_entropy(n_bits, device)
        
        return {
            "success": True,
            "data": result,
        }
    except Exception as e:
        return {
            "success": False,
            "error": str(e),
        }
