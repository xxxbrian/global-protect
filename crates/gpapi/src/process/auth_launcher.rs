use std::process::Stdio;

use anyhow::bail;
use tokio::process::Command;

use crate::{auth::SamlAuthResult, credential::Credential, get_auth_binary};

use super::command_traits::CommandExt;

pub struct SamlAuthLauncher<'a> {
    server: &'a str,
    gateway: bool,
    saml_request: Option<&'a str>,
    user_agent: Option<&'a str>,
    os: Option<&'a str>,
    os_version: Option<&'a str>,
    fix_openssl: bool,
    ignore_tls_errors: bool,
    #[cfg(feature = "webview-auth")]
    hidpi: bool,
    #[cfg(feature = "webview-auth")]
    clean: bool,
    #[cfg(feature = "webview-auth")]
    default_browser: bool,
    browser: Option<&'a str>,
    verbose: Option<&'a str>,
}

impl<'a> SamlAuthLauncher<'a> {
    pub fn new(server: &'a str) -> Self {
        Self {
            server,
            gateway: false,
            saml_request: None,
            user_agent: None,
            os: None,
            os_version: None,
            fix_openssl: false,
            ignore_tls_errors: false,
            #[cfg(feature = "webview-auth")]
            hidpi: false,
            #[cfg(feature = "webview-auth")]
            clean: false,
            #[cfg(feature = "webview-auth")]
            default_browser: false,
            browser: None,
            verbose: None,
        }
    }

    pub fn gateway(mut self, gateway: bool) -> Self {
        self.gateway = gateway;
        self
    }

    pub fn saml_request(mut self, saml_request: &'a str) -> Self {
        self.saml_request = Some(saml_request);
        self
    }

    pub fn user_agent(mut self, user_agent: &'a str) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    pub fn os(mut self, os: &'a str) -> Self {
        self.os = Some(os);
        self
    }

    pub fn os_version(mut self, os_version: Option<&'a str>) -> Self {
        self.os_version = os_version;
        self
    }

    pub fn fix_openssl(mut self, fix_openssl: bool) -> Self {
        self.fix_openssl = fix_openssl;
        self
    }

    pub fn ignore_tls_errors(mut self, ignore_tls_errors: bool) -> Self {
        self.ignore_tls_errors = ignore_tls_errors;
        self
    }

    #[cfg(feature = "webview-auth")]
    pub fn hidpi(mut self, hidpi: bool) -> Self {
        self.hidpi = hidpi;
        self
    }

    #[cfg(feature = "webview-auth")]
    pub fn clean(mut self, clean: bool) -> Self {
        self.clean = clean;
        self
    }

    #[cfg(feature = "webview-auth")]
    pub fn default_browser(mut self, default_browser: bool) -> Self {
        self.default_browser = default_browser;
        self
    }

    pub fn browser(mut self, browser: Option<&'a str>) -> Self {
        self.browser = browser;
        self
    }

    pub fn verbose(mut self, verbose: Option<&'a str>) -> Self {
        self.verbose = verbose;
        self
    }

    /// Launch the authenticator binary as the current user or SUDO_USER if available.
    pub async fn launch(self) -> anyhow::Result<Credential> {
        let mut auth_cmd = Command::new(get_auth_binary());
        auth_cmd.arg(self.server);

        if self.gateway {
            auth_cmd.arg("--gateway");
        }

        if let Some(saml_request) = self.saml_request {
            auth_cmd.arg("--saml-request").arg(saml_request);
        }

        if let Some(user_agent) = self.user_agent {
            auth_cmd.arg("--user-agent").arg(user_agent);
        }

        if let Some(os) = self.os {
            auth_cmd.arg("--os").arg(os);
        }

        if let Some(os_version) = self.os_version {
            auth_cmd.arg("--os-version").arg(os_version);
        }

        if self.fix_openssl {
            auth_cmd.arg("--fix-openssl");
        }

        if self.ignore_tls_errors {
            auth_cmd.arg("--ignore-tls-errors");
        }

        #[cfg(feature = "webview-auth")]
        {
            if self.hidpi {
                auth_cmd.arg("--hidpi");
            }

            if self.clean {
                auth_cmd.arg("--clean");
            }

            if self.default_browser {
                auth_cmd.arg("--default-browser");
            }
        }

        if let Some(browser) = self.browser {
            auth_cmd.arg("--browser").arg(browser);
        }

        if let Some(verbose) = self.verbose {
            auth_cmd.arg(verbose);
        }

        let mut non_root_cmd = auth_cmd.into_non_root()?;
        let output = non_root_cmd
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .await?;

        let Ok(auth_result) = serde_json::from_slice::<SamlAuthResult>(&output.stdout) else {
            bail!("Failed to parse auth data")
        };

        Credential::try_from(auth_result)
    }
}
