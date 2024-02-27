use std::{collections::HashMap, process::exit};

use crate::{
    bail_with_diagnostic,
    ubicloud::{
        Client as UbicloudClient, Credentials as UbicloudCredentials, VmCreateInput, VmState,
    },
    util::{
        compute_resource_state, deserialize_dynamic_value, random_hex_suffix,
        serialize_dynamic_value, IntoDynamicValue, ResourceAction, UNKNOWN_STRING,
    },
};
use serde::{Deserialize, Serialize};
use tonic::{Request, Response, Result};
use tracing::info;

pub mod tf {
    tonic::include_proto!("tfplugin6");
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProviderConfig {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct VmResourceConfig {
    pub region: String,
    pub project_id: String,
    pub name: String,
    pub size: String,
    pub image: String,
    pub user: String,
    pub public_key: String,
    pub enable_public_ipv4: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VmResourceState {
    #[serde(flatten)]
    pub config: VmResourceConfig,

    pub vm_name: String,
    pub public_ipv4: Option<String>,
    pub public_ipv6: Option<String>,
}

#[derive(Debug)]
pub struct UbicloudProvider {
    ubicloud: UbicloudClient,
}

impl UbicloudProvider {
    pub fn new() -> Self {
        Self {
            ubicloud: UbicloudClient::new(None),
        }
    }
}

#[tonic::async_trait]
impl tf::provider_server::Provider for UbicloudProvider {
    async fn get_provider_schema(
        &self,
        request: Request<tf::get_provider_schema::Request>,
    ) -> Result<Response<tf::get_provider_schema::Response>> {
        info!("get_provider_schema: {:?}", request);

        Ok(Response::new(tf::get_provider_schema::Response {
            provider: Some(tf::Schema {
                version: 1,
                block: Some(tf::schema::Block {
                    version: 1,
                    attributes: vec![
                        tf::schema::Attribute {
                            name: "email".to_string(),
                            r#type: "\"string\"".as_bytes().to_vec(),
                            nested_type: None,
                            description: "Email for Ubicloud account used to provision resources"
                                .to_string(),
                            description_kind: tf::StringKind::Plain as i32,
                            required: true,
                            optional: false,
                            computed: false,
                            sensitive: true,
                            deprecated: false,
                        },
                        tf::schema::Attribute {
                            name: "password".to_string(),
                            r#type: "\"string\"".as_bytes().to_vec(),
                            nested_type: None,
                            description:
                                "Password for Ubicloud account used to provision resources"
                                    .to_string(),
                            description_kind: tf::StringKind::Plain as i32,
                            required: true,
                            optional: false,
                            computed: false,
                            sensitive: true,
                            deprecated: false,
                        },
                    ],
                    block_types: vec![],
                    description: "Ubicloud provider".to_string(),
                    description_kind: tf::StringKind::Plain as i32,
                    deprecated: false,
                }),
            }),
            resource_schemas: [(
                "ubicloud_vm".to_string(),
                tf::Schema {
                    version: 1,
                    block: Some(tf::schema::Block {
                        version: 1,
                        attributes: vec![
                            tf::schema::Attribute {
                                name: "region".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "Region where the VM will be created in. Current supported options are `hetzner-hel1` or `hetzner-fsn1`.".to_string(),
                                nested_type: None,
                                required: true,
                                optional: false,
                                computed: false,
                                sensitive: false,
                                description_kind: tf::StringKind::Markdown as i32,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "project_id".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "Project where the VM will be created in.".to_string(),
                                nested_type: None,
                                required: true,
                                optional: false,
                                computed: false,
                                sensitive: false,
                                description_kind: tf::StringKind::Markdown as i32,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "name".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "Friendly name of the resource (used to compute the real name).".to_string(),
                                description_kind: tf::StringKind::Plain as i32,
                                nested_type: None,
                                required: true,
                                optional: false,
                                computed: false,
                                sensitive: false,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "size".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "Size fo the VM. Current supported options are `standard-2`, `standard-4`, `standard-8` and `standard-16`.".to_string(),
                                description_kind: tf::StringKind::Markdown as i32,
                                nested_type: None,
                                required: true,
                                optional: false,
                                computed: false,
                                sensitive: false,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "image".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "Image to use for the VM. Current supported options are `ubuntu-jammy` and `almalinux-9.1`.".to_string(),
                                description_kind: tf::StringKind::Markdown as i32,
                                nested_type: None,
                                required: true,
                                optional: false,
                                computed: false,
                                sensitive: false,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "user".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "Linux user used when creating the VM.".to_string(),
                                description_kind: tf::StringKind::Plain as i32,
                                nested_type: None,
                                required: true,
                                optional: false,
                                computed: false,
                                sensitive: false,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "public_key".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "SSH public key used when creating the VM.".to_string(),
                                description_kind: tf::StringKind::Plain as i32,
                                nested_type: None,
                                required: true,
                                optional: false,
                                computed: false,
                                sensitive: true,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "enable_public_ipv4".to_string(),
                                r#type: String::into_bytes("\"bool\"".to_string()),
                                description: "Whether to enable public IPv4 for the VM. Defaults to `true`.".to_string(),
                                description_kind: tf::StringKind::Markdown as i32,
                                nested_type: None,
                                required: false,
                                optional: true,
                                computed: false,
                                sensitive: false,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "vm_name".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "The real name of the VM in Ubicloud.".to_string(),
                                description_kind: tf::StringKind::Plain as i32,
                                nested_type: None,
                                required: false,
                                optional: true,
                                computed: true,
                                sensitive: false,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "public_ipv4".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "Public IPv4 address of the VM.".to_string(),
                                description_kind: tf::StringKind::Plain as i32,
                                nested_type: None,
                                required: false,
                                optional: true,
                                computed: true,
                                sensitive: false,
                                deprecated: false,
                            },
                            tf::schema::Attribute {
                                name: "public_ipv6".to_string(),
                                r#type: String::into_bytes("\"string\"".to_string()),
                                description: "Public IPv6 address of the VM.".to_string(),
                                description_kind: tf::StringKind::Plain as i32,
                                nested_type: None,
                                required: false,
                                optional: true,
                                computed: true,
                                sensitive: false,
                                deprecated: false,
                            },
                        ],
                        block_types: vec![],
                        description: "Ubicloud Virtual Machine".to_string(),
                        description_kind: tf::StringKind::Plain as i32,
                        deprecated: false,
                    }),
                },
            )]
            .iter()
            .cloned()
            .collect(),
            data_source_schemas: HashMap::new(),
            diagnostics: vec![],
            provider_meta: Some(tf::Schema {
                version: 1,
                block: Some(tf::schema::Block {
                    version: 1,
                    attributes: vec![],
                    block_types: vec![],
                    description: "Ubicloud terraform provider".to_string(),
                    description_kind: tf::StringKind::Markdown as i32,
                    deprecated: false,
                }),
            }),
        }))
    }

    async fn validate_provider_config(
        &self,
        request: Request<tf::validate_provider_config::Request>,
    ) -> Result<Response<tf::validate_provider_config::Response>> {
        info!("validate_provider_config: {:?}", request);

        Ok(Response::new(tf::validate_provider_config::Response {
            diagnostics: vec![],
        }))
    }

    async fn configure_provider(
        &self,
        request: Request<tf::configure_provider::Request>,
    ) -> Result<Response<tf::configure_provider::Response>> {
        let mut response = tf::configure_provider::Response::default();

        info!("configure_provider: {:?}", request);

        let config: Vec<u8> = request.into_inner().config.unwrap().msgpack;
        let Ok(config) = deserialize_dynamic_value::<ProviderConfig>(config) else {
            bail_with_diagnostic!(response, "failed to deserialize configuration");
        };

        info!("config: {:?}", config);

        self.ubicloud
            .set_credentials(UbicloudCredentials {
                email: config.email,
                password: config.password,
            })
            .await;

        Ok(Response::new(response))
    }

    async fn validate_resource_config(
        &self,
        request: Request<tf::validate_resource_config::Request>,
    ) -> Result<Response<tf::validate_resource_config::Response>> {
        info!("validate_resource_config: {:?}", request);

        Ok(Response::new(tf::validate_resource_config::Response {
            diagnostics: vec![],
        }))
    }

    async fn validate_data_resource_config(
        &self,
        request: Request<tf::validate_data_resource_config::Request>,
    ) -> Result<Response<tf::validate_data_resource_config::Response>> {
        info!("validate_data_resource_config: {:?}", request);

        Ok(Response::new(tf::validate_data_resource_config::Response {
            diagnostics: vec![],
        }))
    }

    async fn read_resource(
        &self,
        request: Request<tf::read_resource::Request>,
    ) -> Result<Response<tf::read_resource::Response>> {
        info!("read_resource: {:?}", request);

        let state = request.get_ref().clone().current_state.unwrap().msgpack;

        Ok(Response::new(tf::read_resource::Response {
            new_state: state.into_dynamic_value().into(),
            private: vec![],
            diagnostics: vec![],
        }))
    }

    async fn plan_resource_change(
        &self,
        request: Request<tf::plan_resource_change::Request>,
    ) -> Result<Response<tf::plan_resource_change::Response>> {
        let mut response = tf::plan_resource_change::Response::default();

        info!("plan_resource_change: {:?}", request);

        let Ok(resource_state) = compute_resource_state(
            request.get_ref().clone().prior_state,
            request.get_ref().clone().config,
        ) else {
            bail_with_diagnostic!(response, "failed to compute resource state");
        };

        let planned_state = if !resource_state.did_change {
            let Some(prior_state) = resource_state.prior_state else {
                bail_with_diagnostic!(response, "prior state is missing");
            };
            prior_state
        } else {
            let Some(config) = resource_state.config else {
                bail_with_diagnostic!(response, "prior state is missing");
            };

            VmResourceState {
                config,
                vm_name: UNKNOWN_STRING.to_owned().into(),
                public_ipv4: UNKNOWN_STRING.to_owned().into(),
                public_ipv6: UNKNOWN_STRING.to_owned().into(),
            }
        };

        info!("planned_state: {:?}", planned_state);

        let Ok(planned_state) = serialize_dynamic_value(&planned_state) else {
            bail_with_diagnostic!(response, "failed to serialize planned state");
        };

        Ok(Response::new(tf::plan_resource_change::Response {
            planned_state: planned_state.into_dynamic_value().into(),
            requires_replace: vec![
                "name",
                "region",
                "size",
                "image",
                "user",
                "public_key",
                "enable_public_ipv4",
            ]
            .into_iter()
            .map(|name| tf::AttributePath {
                steps: vec![tf::attribute_path::Step {
                    selector: Some(tf::attribute_path::step::Selector::AttributeName(
                        name.into(),
                    )),
                }],
            })
            .collect(),
            planned_private: vec![],
            diagnostics: vec![],
        }))
    }

    async fn apply_resource_change(
        &self,
        request: Request<tf::apply_resource_change::Request>,
    ) -> Result<Response<tf::apply_resource_change::Response>> {
        let mut response = tf::apply_resource_change::Response::default();

        info!("apply_resource_change: {:?}", request);

        let Ok(resource_state) = compute_resource_state(
            request.get_ref().clone().prior_state,
            request.get_ref().clone().config,
        ) else {
            bail_with_diagnostic!(response, "failed to compute resource state");
        };

        if let ResourceAction::Delete = resource_state.action {
            info!("deleting resource");

            let Some(prior_state) = resource_state.prior_state else {
                bail_with_diagnostic!(response, "prior state is missing");
            };

            let config = prior_state.config.clone();
            let vm_name = prior_state.vm_name.clone();

            let Ok(_) = self
                .ubicloud
                .delete_vm(config.project_id, config.region, vm_name)
                .await
            else {
                bail_with_diagnostic!(response, "failed to delete vm");
            };

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                let config: VmResourceConfig = prior_state.config.clone();
                let vm_name = prior_state.vm_name.clone();

                let Ok(Some(_)) = self
                    .ubicloud
                    .get_vm(config.project_id, config.region, vm_name)
                    .await
                else {
                    break;
                };
            }

            return Ok(Response::new(tf::apply_resource_change::Response {
                new_state: None,
                private: vec![],
                diagnostics: vec![],
            }));
        };

        let planned_state_bytes = request
            .get_ref()
            .clone()
            .planned_state
            .unwrap_or_default()
            .msgpack;
        if planned_state_bytes.len() == 0 {
            bail_with_diagnostic!(response, "planned state is missing");
        };

        let Ok(planned_state) = deserialize_dynamic_value::<VmResourceState>(planned_state_bytes)
        else {
            bail_with_diagnostic!(response, "planned state is missing");
        };

        let config = planned_state.config.clone();
        let vm_name = format!("{}-{}", config.name, random_hex_suffix(3));

        let Ok(_) = self
            .ubicloud
            .create_vm(
                config.project_id,
                config.region,
                VmCreateInput {
                    name: vm_name.clone(),
                    size: config.size,
                    image: config.image,
                    user: config.user,
                    public_key: config.public_key,
                    enable_public_ipv4: config.enable_public_ipv4.unwrap_or(false),
                },
            )
            .await
        else {
            bail_with_diagnostic!(response, "failed to create vm");
        };

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            let config = planned_state.config.clone();

            let Ok(Some(vm)) = self
                .ubicloud
                .get_vm(config.project_id, config.region, vm_name.clone())
                .await
            else {
                bail_with_diagnostic!(response, "failed to get vm");
            };

            match vm.state {
                VmState::Running => break,
                _ => continue,
            };
        }

        let config = planned_state.config.clone();

        let Ok(Some(vm)) = self
            .ubicloud
            .get_vm(config.project_id, config.region, vm_name.clone())
            .await
        else {
            bail_with_diagnostic!(response, "failed to get vm");
        };

        let mut new_state = planned_state.clone();
        new_state.vm_name = vm.name;
        new_state.public_ipv4 = vm.ip4;
        new_state.public_ipv6 = vm.ip6;

        info!("new_state: {:?}", new_state);

        let Ok(new_state) = serialize_dynamic_value(&new_state) else {
            bail_with_diagnostic!(response, "failed to serialize new state");
        };

        Ok(Response::new(tf::apply_resource_change::Response {
            new_state: new_state.into_dynamic_value().into(),
            private: vec![],
            diagnostics: vec![],
        }))
    }

    async fn import_resource_state(
        &self,
        request: Request<tf::import_resource_state::Request>,
    ) -> Result<Response<tf::import_resource_state::Response>> {
        info!("import_resource_state: {:?}", request);
        todo!()
    }

    async fn upgrade_resource_state(
        &self,
        request: Request<tf::upgrade_resource_state::Request>,
    ) -> Result<Response<tf::upgrade_resource_state::Response>> {
        info!("upgrade_resource_state: {:?}", request);
        let raw_state = request.get_ref().raw_state.clone().unwrap();
        let json = String::from_utf8_lossy(raw_state.json.as_slice());
        info!("json: {:?}", json);

        let state = serde_json::from_str::<VmResourceState>(&json).ok();
        info!("state: {:?}", state);

        Ok(Response::new(tf::upgrade_resource_state::Response {
            upgraded_state: Some(tf::DynamicValue {
                msgpack: rmp_serde::to_vec::<VmResourceState>(&state.unwrap()).unwrap(),
                json: vec![],
            }),
            diagnostics: vec![],
        }))
    }

    async fn read_data_source(
        &self,
        request: Request<tf::read_data_source::Request>,
    ) -> Result<Response<tf::read_data_source::Response>> {
        info!("read_data_source: {:?}", request);
        todo!()
    }

    async fn stop_provider(
        &self,
        request: Request<tf::stop_provider::Request>,
    ) -> Result<Response<tf::stop_provider::Response>> {
        info!("stop_provider: {:?}", request);
        exit(0);
    }
}
