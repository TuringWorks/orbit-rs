//! Python runtime with connection pooling and MessagePack communication

use super::config::{PythonConfig, PythonWorkerConfig};
use super::types::{PythonError, PythonResult, PythonValue};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

/// Request to Python worker
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PythonRequest {
    id: usize,
    method: String,
    params: RequestParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum RequestParams {
    Execute(ExecuteParams),
    Batch(BatchParams),
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecuteParams {
    function_source: String,
    function_name: String,
    args: Vec<PythonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchParams {
    requests: Vec<ExecuteParams>,
}

/// Response from Python worker
#[derive(Debug, Clone, Deserialize)]
struct PythonResponse {
    #[allow(dead_code)] // Part of response protocol, may not be used
    id: usize,
    result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
}

/// Single Python worker process
pub struct PythonWorker {
    process: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    execution_count: Arc<Mutex<usize>>,
    config: PythonWorkerConfig,
    use_msgpack: bool,
}

impl PythonWorker {
    /// Spawn a new Python worker process
    pub fn new(python_config: &PythonConfig) -> PythonResult<Self> {
        // Find worker script
        let worker_script = if let Some(ref path) = python_config.worker_script_path {
            path.clone()
        } else {
            // Default: look for worker.py in the same directory as this binary
            let mut path = std::env::current_exe()
                .map_err(|e| PythonError::InternalError(format!("Cannot get exe path: {}", e)))?;
            path.pop(); // Remove binary name
            path.push("worker.py");

            // If not found, try src/python/worker.py (development)
            if !path.exists() {
                path = std::path::PathBuf::from("src/python/worker.py");
            }

            if !path.exists() {
                return Err(PythonError::WorkerError(
                    "Cannot find worker.py script. Set worker_script_path in config.".to_string(),
                ));
            }

            path
        };

        // Spawn Python process
        let mut process = Command::new(&python_config.python_path)
            .arg(&worker_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| PythonError::WorkerError(format!("Failed to spawn Python: {}", e)))?;

        let stdin = Arc::new(Mutex::new(process.stdin.take().ok_or_else(|| {
            PythonError::WorkerError("Failed to get stdin".to_string())
        })?));

        let stdout = Arc::new(Mutex::new(BufReader::new(
            process
                .stdout
                .take()
                .ok_or_else(|| PythonError::WorkerError("Failed to get stdout".to_string()))?,
        )));

        Ok(Self {
            process,
            stdin,
            stdout,
            execution_count: Arc::new(Mutex::new(0)),
            config: python_config.worker.clone(),
            use_msgpack: python_config.use_msgpack,
        })
    }

    /// Execute a single function
    pub fn execute(
        &self,
        func_source: &str,
        func_name: &str,
        args: Vec<PythonValue>,
    ) -> PythonResult<PythonValue> {
        // Increment execution count
        {
            let mut count = self.execution_count.lock().unwrap();
            *count += 1;
        }

        let request = PythonRequest {
            id: 1,
            method: "execute".to_string(),
            params: RequestParams::Execute(ExecuteParams {
                function_source: func_source.to_string(),
                function_name: func_name.to_string(),
                args,
                timeout: Some(self.config.timeout_seconds),
            }),
        };

        let response = if self.use_msgpack {
            self.send_msgpack_request(&request)?
        } else {
            self.send_json_request(&request)?
        };

        self.parse_response(response)
    }

    /// Execute multiple functions in a batch
    pub fn execute_batch(
        &self,
        requests: Vec<(String, String, Vec<PythonValue>)>,
    ) -> PythonResult<Vec<PythonResult<PythonValue>>> {
        let execute_params: Vec<ExecuteParams> = requests
            .into_iter()
            .map(|(source, name, args)| ExecuteParams {
                function_source: source,
                function_name: name,
                args,
                timeout: Some(self.config.timeout_seconds),
            })
            .collect();

        let request = PythonRequest {
            id: 1,
            method: "batch".to_string(),
            params: RequestParams::Batch(BatchParams {
                requests: execute_params,
            }),
        };

        let response = if self.use_msgpack {
            self.send_msgpack_request(&request)?
        } else {
            self.send_json_request(&request)?
        };

        // Parse batch response
        if let Some(error) = response.error {
            return Err(PythonError::RuntimeError(format!("{:?}", error)));
        }

        if let Some(serde_json::Value::Array(results)) = response.result {
            let parsed: Vec<PythonResult<PythonValue>> = results
                .into_iter()
                .map(|r| {
                    if let Some(err) = r.get("error") {
                        if !err.is_null() {
                            return Err(PythonError::RuntimeError(format!("{:?}", err)));
                        }
                    }

                    if let Some(result) = r.get("result") {
                        Ok(serde_json::from_value(result.clone()).unwrap_or(PythonValue::Null))
                    } else {
                        Ok(PythonValue::Null)
                    }
                })
                .collect();

            Ok(parsed)
        } else {
            Err(PythonError::InternalError(
                "Invalid batch response".to_string(),
            ))
        }
    }

    /// Send ping to check worker health
    pub fn ping(&self) -> PythonResult<()> {
        let request = PythonRequest {
            id: 999,
            method: "ping".to_string(),
            params: RequestParams::Empty,
        };

        let response = if self.use_msgpack {
            self.send_msgpack_request(&request)?
        } else {
            self.send_json_request(&request)?
        };

        if response.error.is_some() {
            return Err(PythonError::WorkerError("Ping failed".to_string()));
        }

        Ok(())
    }

    /// Get execution count
    pub fn execution_count(&self) -> usize {
        *self.execution_count.lock().unwrap()
    }

    /// Check if worker should be restarted
    pub fn should_restart(&self) -> bool {
        if self.config.restart_after_executions == 0 {
            return false;
        }

        self.execution_count() >= self.config.restart_after_executions
    }

    fn send_json_request(&self, request: &PythonRequest) -> PythonResult<PythonResponse> {
        let request_json = serde_json::to_string(request)
            .map_err(|e| PythonError::CommunicationError(format!("JSON encode error: {}", e)))?;

        // Write request
        {
            let mut stdin = self.stdin.lock().unwrap();
            writeln!(stdin, "{}", request_json)
                .map_err(|e| PythonError::CommunicationError(format!("Write error: {}", e)))?;
            stdin
                .flush()
                .map_err(|e| PythonError::CommunicationError(format!("Flush error: {}", e)))?;
        }

        // Read response
        let response_line = {
            let mut stdout = self.stdout.lock().unwrap();
            let mut line = String::new();
            std::io::BufRead::read_line(&mut *stdout, &mut line)
                .map_err(|e| PythonError::CommunicationError(format!("Read error: {}", e)))?;
            line
        };

        serde_json::from_str(&response_line)
            .map_err(|e| PythonError::CommunicationError(format!("JSON decode error: {}", e)))
    }

    fn send_msgpack_request(&self, request: &PythonRequest) -> PythonResult<PythonResponse> {
        // Serialize request
        let packed = rmp_serde::to_vec(request).map_err(|e| {
            PythonError::CommunicationError(format!("MessagePack encode error: {}", e))
        })?;

        // Write request
        {
            let mut stdin = self.stdin.lock().unwrap();
            stdin
                .write_all(&packed)
                .map_err(|e| PythonError::CommunicationError(format!("Write error: {}", e)))?;
            stdin
                .flush()
                .map_err(|e| PythonError::CommunicationError(format!("Flush error: {}", e)))?;
        }

        // Read length prefix (4 bytes)
        let mut length_bytes = [0u8; 4];
        {
            let mut stdout = self.stdout.lock().unwrap();
            stdout.read_exact(&mut length_bytes).map_err(|e| {
                PythonError::CommunicationError(format!("Read length error: {}", e))
            })?;
        }

        let length = u32::from_be_bytes(length_bytes) as usize;

        // Read response data
        let mut response_bytes = vec![0u8; length];
        {
            let mut stdout = self.stdout.lock().unwrap();
            stdout
                .read_exact(&mut response_bytes)
                .map_err(|e| PythonError::CommunicationError(format!("Read data error: {}", e)))?;
        }

        // Deserialize response
        rmp_serde::from_slice(&response_bytes).map_err(|e| {
            PythonError::CommunicationError(format!("MessagePack decode error: {}", e))
        })
    }

    fn parse_response(&self, response: PythonResponse) -> PythonResult<PythonValue> {
        if let Some(error) = response.error {
            return Err(PythonError::RuntimeError(format!("{:?}", error)));
        }

        response
            .result
            .map(|r| serde_json::from_value(r).unwrap_or(PythonValue::Null))
            .ok_or_else(|| PythonError::InternalError("No result in response".to_string()))
    }
}

impl Drop for PythonWorker {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// Pool of Python worker processes
pub struct PythonRuntimePool {
    workers: Arc<RwLock<Vec<Arc<Mutex<PythonWorker>>>>>,
    next_worker: Arc<Mutex<usize>>,
    config: PythonConfig,
}

impl PythonRuntimePool {
    /// Create a new runtime pool
    pub async fn new(config: PythonConfig) -> PythonResult<Self> {
        let mut workers = Vec::new();

        for _ in 0..config.pool_size {
            let worker = PythonWorker::new(&config)?;
            workers.push(Arc::new(Mutex::new(worker)));
        }

        Ok(Self {
            workers: Arc::new(RwLock::new(workers)),
            next_worker: Arc::new(Mutex::new(0)),
            config,
        })
    }

    /// Execute a function (round-robin worker selection)
    pub async fn execute(
        &self,
        func_source: &str,
        func_name: &str,
        args: Vec<PythonValue>,
    ) -> PythonResult<PythonValue> {
        let worker = self.get_next_worker().await?;

        // Execute in blocking task to avoid blocking async runtime
        let func_source = func_source.to_string();
        let func_name = func_name.to_string();

        tokio::task::spawn_blocking(move || {
            let worker_guard = worker.lock().unwrap();
            worker_guard.execute(&func_source, &func_name, args)
        })
        .await
        .map_err(|e| PythonError::InternalError(format!("Task join error: {}", e)))?
    }

    /// Execute multiple functions in batch
    pub async fn execute_batch(
        &self,
        requests: Vec<(String, String, Vec<PythonValue>)>,
    ) -> PythonResult<Vec<PythonResult<PythonValue>>> {
        let worker = self.get_next_worker().await?;

        tokio::task::spawn_blocking(move || {
            let worker_guard = worker.lock().unwrap();
            worker_guard.execute_batch(requests)
        })
        .await
        .map_err(|e| PythonError::InternalError(format!("Task join error: {}", e)))?
    }

    async fn get_next_worker(&self) -> PythonResult<Arc<Mutex<PythonWorker>>> {
        let workers = self.workers.read().await;

        if workers.is_empty() {
            return Err(PythonError::WorkerError("No workers available".to_string()));
        }

        // Round-robin selection
        let worker_idx = {
            let mut next = self.next_worker.lock().unwrap();
            let idx = *next;
            *next = (*next + 1) % workers.len();
            idx
        };

        let worker = workers[worker_idx].clone();

        // Check if worker should be restarted
        {
            let worker_guard = worker.lock().unwrap();
            if worker_guard.should_restart() {
                drop(worker_guard);
                drop(workers);

                // Replace worker
                self.replace_worker(worker_idx).await?;

                let workers = self.workers.read().await;
                return Ok(workers[worker_idx].clone());
            }
        }

        Ok(worker)
    }

    async fn replace_worker(&self, idx: usize) -> PythonResult<()> {
        let new_worker = PythonWorker::new(&self.config)?;

        let mut workers = self.workers.write().await;
        if idx < workers.len() {
            workers[idx] = Arc::new(Mutex::new(new_worker));
        }

        Ok(())
    }

    /// Health check all workers
    pub async fn health_check(&self) -> Vec<bool> {
        let workers = self.workers.read().await;
        let mut results = Vec::new();

        for worker in workers.iter() {
            let worker_guard = worker.lock().unwrap();
            let healthy = worker_guard.ping().is_ok();
            results.push(healthy);
        }

        results
    }
}

/// Single Python runtime (non-pooled)
pub struct PythonRuntime {
    worker: Arc<Mutex<PythonWorker>>,
}

impl PythonRuntime {
    /// Create a new single-worker runtime
    pub fn new(config: &PythonConfig) -> PythonResult<Self> {
        let worker = PythonWorker::new(config)?;
        Ok(Self {
            worker: Arc::new(Mutex::new(worker)),
        })
    }

    /// Execute a function
    pub async fn execute(
        &self,
        func_source: &str,
        func_name: &str,
        args: Vec<PythonValue>,
    ) -> PythonResult<PythonValue> {
        let worker = self.worker.clone();
        let func_source = func_source.to_string();
        let func_name = func_name.to_string();

        tokio::task::spawn_blocking(move || {
            let worker_guard = worker.lock().unwrap();
            worker_guard.execute(&func_source, &func_name, args)
        })
        .await
        .map_err(|e| PythonError::InternalError(format!("Task join error: {}", e)))?
    }
}
