pub mod auth;
pub mod credential;
pub mod error;
pub mod gateway;
pub mod gp_params;
pub mod portal;
pub mod process;
pub mod service;
pub mod utils;

#[cfg(feature = "logger")]
pub mod logger;

#[cfg(feature = "clap")]
pub mod clap;

#[cfg(debug_assertions)]
pub const GP_API_KEY: &[u8; 32] = &[0; 32];

pub const GP_USER_AGENT: &str = "PAN GlobalProtect";
pub const GP_SERVICE_LOCK_FILE: &str = "/var/run/gpservice.lock";
pub const GP_CALLBACK_PORT_FILENAME: &str = "gpcallback.port";

#[cfg(not(debug_assertions))]
use std::path::PathBuf;
#[cfg(not(debug_assertions))]
use which::which;

pub fn get_binary_path(binary_name: &str) -> String {
    #[cfg(debug_assertions)]
    {
        match binary_name {
            "gpclient" => env!("GP_CLIENT_BINARY").to_string(),
            "gpservice" => env!("GP_SERVICE_BINARY").to_string(),
            "gpauth" => env!("GP_AUTH_BINARY").to_string(),
            _ => format!("/usr/bin/{}", binary_name),
        }
    }

    #[cfg(not(debug_assertions))]
    {
        if let Ok(path) = which(binary_name) {
            return path.to_string_lossy().into_owned();
        }

        let default_base = if cfg!(target_os = "macos") {
            "/opt/homebrew/bin"
        } else {
            "/usr/bin"
        };

        PathBuf::from(default_base)
            .join(binary_name)
            .to_string_lossy()
            .into_owned()
    }
}

pub fn get_client_binary() -> String {
    get_binary_path("gpclient")
}

pub fn get_service_binary() -> String {
    get_binary_path("gpservice")
}

pub fn get_auth_binary() -> String {
    get_binary_path("gpauth")
}
