//! Azure Quantum runner using the `az` CLI.
//!
//! Implements [`QuantumRunner`] by exporting circuits to QIR via
//! `roqoqo-qir`, submitting jobs through `az quantum job submit`, and
//! parsing the histogram results.
//!
//! Requires `az` CLI installed and authenticated (`az login`).

use algos::runner::{BitRegisters, QuantumRunner};
use roqoqo::Circuit;
use roqoqo_qir::Backend as QirBackend;
use std::collections::HashMap;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

/// Runs quantum circuits on Azure Quantum via the `az` CLI.
///
/// Exports circuits to QIR (Quantum Intermediate Representation) using
/// `roqoqo-qir`, then submits via `az quantum job submit`. Works with
/// any QIR-compatible provider (Quantinuum, Rigetti, IonQ).
///
/// # Example
/// ```no_run
/// use examples::azure_runner::AzureCliRunner;
/// use algos::grover::{search, GroverConfig};
///
/// let runner = AzureCliRunner::new("qcfront-ws", "qcfront-rg", "quantinuum.sim.h2-1e");
/// let config = GroverConfig { num_qubits: 3, num_shots: 100, ..Default::default() };
/// let result = search(&config, 5, &runner);
/// ```
pub struct AzureCliRunner {
    workspace: String,
    resource_group: String,
    target: String,
}

impl AzureCliRunner {
    pub fn new(workspace: &str, resource_group: &str, target: &str) -> Self {
        Self {
            workspace: workspace.to_string(),
            resource_group: resource_group.to_string(),
            target: target.to_string(),
        }
    }

    /// Export a roqoqo circuit to QIR (LLVM IR text).
    fn circuit_to_qir(&self, circuit: &Circuit) -> String {
        let backend = QirBackend::new(None, None).expect("Failed to create QIR backend");
        backend
            .circuit_to_qir_str(circuit, true)
            .expect("Failed to export circuit to QIR")
    }

    /// Submit a QIR file to Azure Quantum and return the job ID.
    fn submit_job(&self, qir_path: &str, shots: usize) -> String {
        let output = Command::new("az")
            .args([
                "quantum",
                "job",
                "submit",
                "-w",
                &self.workspace,
                "-g",
                &self.resource_group,
                "-t",
                &self.target,
                "--job-name",
                "qcfront-job",
                "--job-input-file",
                qir_path,
                "--job-input-format",
                "qir.v1",
                "--job-output-format",
                "microsoft.quantum-results.v1",
                "--entry-point",
                "main",
                "--job-params",
                &format!("count={}", shots),
                "--output",
                "json",
            ])
            .output()
            .expect("Failed to execute 'az quantum job submit'. Is az CLI installed?");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("az quantum job submit failed: {}", stderr);
        }

        let json: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("Failed to parse job submit response");

        json["id"]
            .as_str()
            .expect("No job ID in submit response")
            .to_string()
    }

    /// Poll for job completion.
    fn wait_for_job(&self, job_id: &str) {
        loop {
            let output = Command::new("az")
                .args([
                    "quantum",
                    "job",
                    "show",
                    "-w",
                    &self.workspace,
                    "-g",
                    &self.resource_group,
                    "--job-id",
                    job_id,
                    "--query",
                    "status",
                    "-o",
                    "tsv",
                ])
                .output()
                .expect("Failed to execute 'az quantum job show'");

            let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
            match status.as_str() {
                "Succeeded" => return,
                "Failed" | "Cancelled" => {
                    panic!("Azure Quantum job {}: {}", status, job_id);
                }
                _ => {
                    thread::sleep(Duration::from_secs(2));
                }
            }
        }
    }

    /// Fetch job output histogram.
    fn get_job_output(&self, job_id: &str) -> serde_json::Value {
        let output = Command::new("az")
            .args([
                "quantum",
                "job",
                "output",
                "-w",
                &self.workspace,
                "-g",
                &self.resource_group,
                "--job-id",
                job_id,
                "-o",
                "json",
            ])
            .output()
            .expect("Failed to execute 'az quantum job output'");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("az quantum job output failed: {}", stderr);
        }

        serde_json::from_slice(&output.stdout).expect("Failed to parse job output")
    }

    /// Parse Azure Quantum histogram into BitRegisters.
    ///
    /// Azure returns histogram as alternating [bitstring, probability] pairs:
    /// `{"Histogram": ["[0, 0]", 0.5, "[1, 1]", 0.5]}`
    ///
    /// We expand probabilities into per-shot bit vectors.
    fn parse_histogram(&self, output: &serde_json::Value, shots: usize) -> BitRegisters {
        let histogram = output
            .get("Histogram")
            .or_else(|| output.get("histogram"))
            .and_then(|h| h.as_array())
            .expect("No histogram array in job output");

        let mut all_shots: Vec<Vec<bool>> = Vec::new();

        // Parse alternating pairs: bitstring, probability, bitstring, probability, ...
        let mut i = 0;
        while i + 1 < histogram.len() {
            let bitstring = histogram[i].as_str().unwrap_or("[]");
            let probability = histogram[i + 1].as_f64().unwrap_or(0.0);

            // Parse "[0, 1, 0]" → vec![false, true, false]
            let bits: Vec<bool> = bitstring
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|s| s.trim() == "1")
                .collect();

            // Convert probability to count
            let count = (probability * shots as f64).round() as usize;
            for _ in 0..count {
                all_shots.push(bits.clone());
            }

            i += 2;
        }

        let mut registers: BitRegisters = HashMap::new();
        registers.insert("result".to_string(), all_shots);
        registers
    }
}

impl QuantumRunner for AzureCliRunner {
    fn run(&self, circuit: &Circuit, shots: usize) -> BitRegisters {
        // 1. Export to QIR and write to temp file
        let qir = self.circuit_to_qir(circuit);

        let mut tmp = tempfile::Builder::new()
            .suffix(".ll")
            .tempfile()
            .expect("Failed to create temp file");
        tmp.write_all(qir.as_bytes())
            .expect("Failed to write QIR to temp file");
        let tmp_path = tmp.path().to_str().unwrap().to_string();

        eprintln!(
            "[AzureCliRunner] Submitting {} shots to {}",
            shots, self.target
        );

        // 2. Submit job
        let job_id = self.submit_job(&tmp_path, shots);
        eprintln!("[AzureCliRunner] Job submitted: {}", job_id);

        // 3. Wait for completion
        self.wait_for_job(&job_id);
        eprintln!("[AzureCliRunner] Job completed");

        // 4. Get output and parse histogram
        let output = self.get_job_output(&job_id);
        self.parse_histogram(&output, shots)
    }
}
