use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const UBICLOUD_BASE_URL: &str = "https://console.ubicloud.com/api";

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
pub enum VmState {
    #[serde(rename = "creating")]
    Creating,

    #[serde(rename = "running")]
    Running,

    #[serde(rename = "deleting")]
    Deleting,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vm {
    pub id: String,
    pub name: String,
    pub state: VmState,
    pub location: String,

    #[serde(rename = "display_size")]
    pub size: String,

    #[serde(rename = "unix_user")]
    pub user: String,

    pub ip4: Option<String>,
    pub ip6: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VmCreateInput {
    pub name: String,
    pub size: String,
    pub public_key: String,

    #[serde(rename = "unix_user")]
    pub user: String,

    #[serde(rename = "boot_image")]
    pub image: String,

    #[serde(rename = "enable_ip4")]
    pub enable_public_ipv4: bool,
}

#[derive(Debug)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

#[derive(Debug)]
pub struct Client {
    client: reqwest::Client,
    credentials: Mutex<Option<Credentials>>,
    token: Mutex<Option<String>>,
}

impl Client {
    pub fn new(credentials: Option<Credentials>) -> Self {
        Self {
            client: reqwest::Client::new(),
            credentials: Mutex::new(credentials),
            token: Mutex::new(None),
        }
    }

    pub async fn set_credentials(&self, creds: Credentials) {
        let mut credentials = self.credentials.lock().await;
        credentials.replace(creds);
    }

    async fn login(&self) -> Result<()> {
        let mut token: tokio::sync::MutexGuard<'_, Option<String>> = self.token.lock().await;
        let credentials = self.credentials.lock().await;

        if credentials.is_none() {
            bail!("ubicloud credentials not set");
        };

        let credentials = credentials.as_ref().unwrap();

        let client = reqwest::Client::new();
        let url = format!(
            "{}/login?login={}&password={}",
            UBICLOUD_BASE_URL, credentials.email, credentials.password
        );
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("ubicloud login failed");
        }

        let Some(token_header) = response.headers().get("authorization") else {
            bail!("ubicloud login failed");
        };

        token.replace(String::from_utf8_lossy(token_header.as_bytes()).to_string());

        Ok(())
    }

    async fn ensure_auth(&self) -> Result<()> {
        let has_token = {
            let token = self.token.lock().await;
            token.is_some()
        };
        if !has_token {
            self.login().await?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn list_vm(&self, project_id: String, location: String) -> Result<Vec<Vm>> {
        self.ensure_auth().await?;
        let token = {
            let token = self.token.lock().await;
            token.clone().unwrap()
        };

        let url = format!("{UBICLOUD_BASE_URL}/project/{project_id}/location/{location}/vm");

        let response = self
            .client
            .get(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", token)
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("ubicloud list vm failed");
        }

        let vms: String = response.text().await?;
        let vms: Vec<Vm> = serde_json::from_str(&vms)?;

        Ok(vms)
    }

    pub async fn get_vm(
        &self,
        project_id: String,
        location: String,
        name: String,
    ) -> Result<Option<Vm>> {
        self.ensure_auth().await?;
        let token = {
            let token = self.token.lock().await;
            token.clone().unwrap()
        };

        let url = format!("{UBICLOUD_BASE_URL}/project/{project_id}/location/{location}/vm/{name}");

        let response = self
            .client
            .get(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", token)
            .send()
            .await?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            bail!("ubicloud get vm failed");
        }

        let vm: String = response.text().await?;
        let vm: Vm = serde_json::from_str(&vm)?;

        Ok(Some(vm))
    }

    pub async fn delete_vm(
        &self,
        project_id: String,
        location: String,
        name: String,
    ) -> Result<()> {
        self.ensure_auth().await?;
        let token = {
            let token = self.token.lock().await;
            token.clone().unwrap()
        };

        let url = format!("{UBICLOUD_BASE_URL}/project/{project_id}/location/{location}/vm/{name}");

        let response = self
            .client
            .delete(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", token)
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("ubicloud delete vm failed");
        }

        Ok(())
    }

    pub async fn create_vm(
        &self,
        project_id: String,
        location: String,
        input: VmCreateInput,
    ) -> Result<Vm> {
        self.ensure_auth().await?;
        let token = {
            let token = self.token.lock().await;
            token.clone().unwrap()
        };

        let url = format!("{UBICLOUD_BASE_URL}/project/{project_id}/location/{location}/vm");

        let input = serde_json::to_string(&input)?;

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", token)
            .body(input)
            .send()
            .await?;

        if !response.status().is_success() {
            bail!(
                "ubicloud create vm failed with status {} {}",
                response.status(),
                response.text().await?
            );
        }

        let text: String = response.text().await?;
        let vm: Vm = serde_json::from_str(&text)
            .map_err(|e| anyhow!("failed to deserialize: {} {}", e, &text))?;

        Ok(vm)
    }
}
