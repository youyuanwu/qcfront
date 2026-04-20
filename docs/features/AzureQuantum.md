# Azure Quantum: Cloud Quantum Execution

## Overview

Azure Quantum provides free cloud-hosted quantum simulators from multiple
hardware providers. The `AzureCliRunner` implements [`QuantumRunner`] to
run roqoqo circuits on these backends via the `az` CLI.

Verified working: Bell state on Quantinuum H2 emulator — 99% correlated
outcomes with realistic noise modeling.

## Free Resources

| Resource | Cost | Notes |
|----------|------|-------|
| Quantinuum Emulator (h2-1e) | Free | Noisy emulator modeling H-Series hardware |
| Quantinuum Syntax Checker (h2-1sc) | Free | Validates circuits, deterministic results |
| Rigetti QVM | Free | Quil-based simulator (requires Quil/QIR input) |
| IonQ Simulator | Free | Noiseless trapped-ion sim (requires payment method on file) |
| Hardware credits | $500 free/provider | One-time grant per provider |

## Setup

### 1. Prerequisites

```bash
az login
az extension add -n quantum
az provider register -n Microsoft.Quantum
az provider register -n Microsoft.Solutions
```

### 2. Accept Provider Terms

```bash
az quantum offerings accept-terms \
  --provider-id rigetti --location westus \
  --sku azure-basic-qvm-only-unlimited

# IonQ requires a payment method on the Azure subscription:
# az quantum offerings accept-terms \
#   --provider-id ionq --location westus --sku aqt-pay-as-you-go
```

List available providers/SKUs:
```bash
az quantum offerings list --location westus -o table
```

### 3. Create Quantum Workspace

```bash
az group create --name qcfront-rg --location westus

az storage account create \
  --name qcfrontstorage \
  --resource-group qcfront-rg \
  --location westus \
  --sku Standard_LRS

az quantum workspace create \
  --workspace-name qcfront-ws \
  --resource-group qcfront-rg \
  --location westus \
  --storage-account qcfrontstorage
```

### 4. Verify Targets

```bash
az quantum target list \
  -w qcfront-ws -g qcfront-rg -o table
```

## Architecture

### Circuit Export: QIR via roqoqo-qir

```
roqoqo Circuit → roqoqo-qir → QIR (.ll file) → az CLI → provider
```

QIR (Quantum Intermediate Representation) is the universal format for
Azure Quantum. We use `roqoqo-qir` to export LLVM IR text with
`measure_all: true` (required for result recording).

OpenQASM does not work via the `az` CLI — providers require their native
format or QIR. The CLI cannot translate OpenQASM for Quantinuum or Rigetti.

### Result Format

Azure returns histogram probabilities as alternating pairs:
```json
{"Histogram": ["[0, 0]", 0.40, "[1, 1]", 0.59, "[0, 1]", 0.01]}
```

The `AzureCliRunner` expands these into per-shot bit vectors for
`BitRegisters` compatibility.

## Usage

```bash
# Run Bell state on Quantinuum emulator
cargo run -p examples --bin azure_bell -- \
  --workspace qcfront-ws \
  --resource-group qcfront-rg \
  --target quantinuum.sim.h2-1e \
  --shots 100
```

Or use `AzureCliRunner` as a `QuantumRunner` in code:
```rust
let runner = AzureCliRunner::new("qcfront-ws", "qcfront-rg", "quantinuum.sim.h2-1e");
let result = grover::search(&config, target, &runner);
```

## Available Targets

| Target | Provider | Type | Input Formats |
|--------|----------|------|---------------|
| `quantinuum.sim.h2-1sc` | Quantinuum | Syntax checker | QIR |
| `quantinuum.sim.h2-1e` | Quantinuum | Noisy emulator | QIR |
| `rigetti.sim.qvm` | Rigetti | QVM simulator | Quil, QIR |
| `ionq.simulator` | IonQ | Noiseless sim | IonQ JSON, QIR |
| `ionq.qpu.forte-1` | IonQ | Real QPU | IonQ JSON, QIR |

## Future Work

**Phase 2: REST API runner** — Direct HTTP calls via `reqwest` +
`azure_identity` for environments without `az` CLI. Same `QuantumRunner`
interface, no subprocess spawning.
