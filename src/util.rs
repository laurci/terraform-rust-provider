use anyhow::Result;
use rand::Rng;
use rmp::Marker;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    cty::{decode_unknown_string_values, encode_unknown_string_values},
    server::{tf, VmResourceConfig, VmResourceState},
};

pub const UNKNOWN_STRING: &'static str = "<unknown>";

pub fn random_hex_suffix(len: usize) -> String {
    let mut rng = rand::thread_rng();
    let mut suffix = String::with_capacity(len);

    for _ in 0..len {
        suffix.push_str(&format!("{:x}", rng.gen::<u8>()));
    }

    suffix
}

#[macro_export]
macro_rules! bail_with_diagnostic {
    ($resp:ident, $summary:expr, $detail:expr, $severity:expr) => {
        $resp.diagnostics.push(crate::server::tf::Diagnostic {
            severity: $severity as i32,
            summary: $summary.to_string(),
            detail: $detail.to_string(),
            ..Default::default()
        });

        return Ok(tonic::Response::new($resp));
    };
    ($resp:ident, $summary:expr, $detail:expr) => {
        bail_with_diagnostic!(
            $resp,
            $summary,
            $detail,
            crate::server::tf::diagnostic::Severity::Error
        );
    };
    ($resp:ident, $summary:expr) => {
        bail_with_diagnostic!(
            $resp,
            $summary,
            $summary,
            crate::server::tf::diagnostic::Severity::Error
        );
    };
}

pub trait IntoDynamicValue {
    fn into_dynamic_value(self) -> tf::DynamicValue;
}

impl IntoDynamicValue for Vec<u8> {
    fn into_dynamic_value(self) -> tf::DynamicValue {
        tf::DynamicValue {
            msgpack: self,
            json: vec![],
        }
    }
}

pub enum ResourceAction {
    Create,
    Delete,
}

pub struct ResourceState {
    pub already_exists: bool,
    pub did_change: bool,
    pub action: ResourceAction,
    pub prior_state: Option<VmResourceState>,
    pub config: Option<VmResourceConfig>,
}

pub fn deserialize_dynamic_value<T>(data: Vec<u8>) -> Result<T>
where
    T: DeserializeOwned,
{
    let data = decode_unknown_string_values(data)?;
    let data = rmp_serde::from_slice::<T>(data.as_slice())?;

    Ok(data)
}

pub fn serialize_dynamic_value<T>(data: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let data = rmp_serde::to_vec(data)?;
    let data = encode_unknown_string_values(data)?;

    Ok(data)
}

pub fn compute_resource_state(
    prior_state: Option<tf::DynamicValue>,
    config: Option<tf::DynamicValue>,
) -> Result<ResourceState> {
    let prior_state_bytes = prior_state
        .unwrap_or(tf::DynamicValue {
            msgpack: vec![Marker::Null.to_u8()],
            json: vec![],
        })
        .msgpack;

    let prior_state_exists =
        prior_state_bytes.len() > 1 && prior_state_bytes[0] != Marker::Null.to_u8();

    let prior_state = if prior_state_exists {
        Some(deserialize_dynamic_value::<VmResourceState>(
            prior_state_bytes.clone(),
        )?)
    } else {
        None
    };

    let config_bytes = config
        .unwrap_or(tf::DynamicValue {
            msgpack: vec![Marker::Null.to_u8()],
            json: vec![],
        })
        .msgpack;

    let config_exists = config_bytes.len() > 1 && config_bytes[0] != Marker::Null.to_u8();

    let config = if config_exists {
        Some(deserialize_dynamic_value::<VmResourceConfig>(
            config_bytes.clone(),
        )?)
    } else {
        None
    };

    let did_config_change = if !prior_state_exists || !config_exists {
        true
    } else {
        let prior_state = deserialize_dynamic_value::<VmResourceState>(prior_state_bytes)?;
        let config = deserialize_dynamic_value::<VmResourceConfig>(config_bytes)?;

        config != prior_state.config
    };

    let action = if prior_state_exists && !config_exists {
        ResourceAction::Delete
    } else if !prior_state_exists && config_exists {
        ResourceAction::Create
    } else {
        ResourceAction::Create
    };

    Ok(ResourceState {
        already_exists: prior_state_exists,
        did_change: did_config_change,
        action,
        prior_state,
        config,
    })
}
