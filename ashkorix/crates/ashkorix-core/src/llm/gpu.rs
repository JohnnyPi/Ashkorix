use std::sync::OnceLock;

use llama_cpp_2::{list_llama_ggml_backend_devices, LlamaBackendDevice, LlamaBackendDeviceType};
use serde::{Deserialize, Serialize};

use crate::llm::backend::shared_llama_backend;

/// Runtime CUDA availability detected via llama.cpp backend devices.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CudaStatus {
    /// Whether this binary was compiled with the `cuda` feature.
    pub compiled: bool,
    /// Whether at least one CUDA GPU was detected at startup.
    pub available: bool,
    /// Primary CUDA device description, when available.
    pub device_name: Option<String>,
}

static CUDA_STATUS: OnceLock<CudaStatus> = OnceLock::new();

/// Cached CUDA status for the process (detected once after backend init).
pub fn cuda_status() -> CudaStatus {
    CUDA_STATUS.get_or_init(detect_cuda_status).clone()
}

fn detect_cuda_status() -> CudaStatus {
    let compiled = cfg!(feature = "cuda");
    if !compiled {
        return CudaStatus {
            compiled: false,
            available: false,
            device_name: None,
        };
    }

    if shared_llama_backend().is_err() {
        return CudaStatus {
            compiled: true,
            available: false,
            device_name: None,
        };
    }

    let devices: Vec<LlamaBackendDevice> = list_llama_ggml_backend_devices()
        .into_iter()
        .filter(is_cuda_device)
        .collect();

    CudaStatus {
        compiled: true,
        available: !devices.is_empty(),
        device_name: devices.first().map(|d| d.description.clone()),
    }
}

fn is_cuda_device(device: &LlamaBackendDevice) -> bool {
    device.backend.eq_ignore_ascii_case("cuda")
        && matches!(
            device.device_type,
            LlamaBackendDeviceType::Gpu | LlamaBackendDeviceType::Accelerator
        )
}

/// Resolve effective GPU layer offload count.
///
/// `config_layers == 0` means auto: offload all layers when CUDA is available, otherwise CPU.
/// Any positive value is treated as an explicit override.
pub fn resolve_gpu_layers(config_layers: u32) -> u32 {
    if config_layers > 0 {
        return config_layers;
    }
    if cuda_status().available {
        // llama.cpp offloads all layers when n_gpu_layers exceeds model layer count.
        999
    } else {
        0
    }
}
