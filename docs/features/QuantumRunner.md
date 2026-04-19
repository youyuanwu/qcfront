# QuantumRunner: Backend Abstraction

## Overview

The `QuantumRunner` trait decouples algorithm implementations from specific
quantum backends. Algorithms accept `&impl QuantumRunner` instead of raw
closures, matching Qiskit's pattern where a reusable backend object handles
circuit execution.

The trait is defined in the `algos` crate (no backend dependencies). The
`QuestRunner` implementation lives in the `examples` crate.

## Design Rationale

### Shot Batching

The runner receives `shots` as a parameter and returns all results at once.
This is critical for hardware backends — submitting one job with 1000 shots
is fundamentally different from 1000 individual submissions. Local simulators
(like QuEST) loop internally.

### No `num_qubits` Parameter

roqoqo's `Circuit::number_of_qubits()` derives the count from the circuit
by scanning `max(qubit_index) + 1`. Runners call this internally, matching
Qiskit where `backend.run(circuit, shots=N)` never requires an explicit
qubit count.

### Trait Object Support

Algorithm functions use `<R: QuantumRunner + ?Sized>` to support both
static dispatch (`&QuestRunner`) and dynamic dispatch (`&dyn QuantumRunner`).

### Closure Compatibility

A blanket impl allows any `Fn(&Circuit, usize) -> BitRegisters` to be used
as a runner for ad-hoc or test scenarios.

## Comparison with Other Frameworks

| Concept | Qiskit | Cirq | qcfront |
|---------|--------|------|---------|
| Backend object | `Backend` | `Simulator` | `QuantumRunner` |
| Execute | `backend.run(circuit, shots)` | `simulator.simulate(circuit)` | `runner.run(&circuit, shots)` |
| Simulator | `AerSimulator()` | `cirq.Simulator()` | `QuestRunner` |
| Cloud | `IBMBackend(name)` | `cirq_google.Engine()` | `BraketRunner` / `IqmRunner` |
| Shot batching | Built-in (1 job) | `repetitions=N` | Built-in (1 call) |

## Future Extensions

### Error Handling

Current design panics on backend errors. A `FallibleRunner` trait with
`Result`-based `try_run` could be added alongside `QuantumRunner` for
hardware backends where network/device errors are expected.

### Hardware Runners

The same trait works for cloud hardware (AWS Braket, IQM, AQT). Each
runner would convert roqoqo circuits to the provider's format, submit jobs
via REST API, and parse results back into `BitRegisters`.

### Runner Composition

Decorator pattern for cross-cutting concerns: `LoggingRunner<R>`,
`RetryRunner<R>`, `NoisyRunner<R>` wrapping any inner `QuantumRunner`.

### Device Awareness

A `DeviceAwareRunner` sub-trait could expose hardware topology (max qubits,
native gates, connectivity graph) for circuit validation before submission.
