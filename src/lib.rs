// Copyright (C) Nicolas Lamirault <nicolas.lamirault@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use lazy_static::lazy_static;

use guest::prelude::*;
use kubewarden_policy_sdk::wapc_guest as guest;

use k8s_openapi::api::core::v1 as apicore;

extern crate kubewarden_policy_sdk as kubewarden;
use kubewarden::{logging, protocol_version_guest, request::ValidationRequest, validate_settings};

mod settings;
use settings::Settings;

use slog::{info, o, warn, Logger};

lazy_static! {
    static ref LOG_DRAIN: Logger = Logger::root(
        logging::KubewardenDrain::new(),
        o!("policy" => "probes-policy")
    );
}

#[no_mangle]
pub extern "C" fn wapc_init() {
    register_function("validate", validate);
    register_function("validate_settings", validate_settings::<Settings>);
    register_function("protocol_version", protocol_version_guest);
}

fn validate_container(container: &apicore::Container) -> Result<()> {
    if container.liveness_probe.is_none() {
        info!(
            LOG_DRAIN,
            "rejecting pod";
            "container_name" => &container.name
        );
        return Err(anyhow!(
            "container {} without liveness probe is not accepted",
            &container.name
        ));
    }
    if container.readiness_probe.is_none() {
        info!(
            LOG_DRAIN,
            "rejecting pod";
            "container_name" => &container.name
        );
        return Err(anyhow!(
            "container {} without readiness probe is not accepted",
            &container.name
        ));
    }
    Ok(())
}

fn validate_ephemeral_container(container: &apicore::EphemeralContainer) -> Result<()> {
    if container.liveness_probe.is_none() {
        info!(
            LOG_DRAIN,
            "rejecting pod";
            "container_name" => &container.name
        );
        return Err(anyhow!(
            "container {} without liveness probe is not accepted",
            &container.name
        ));
    }
    if container.readiness_probe.is_none() {
        info!(
            LOG_DRAIN,
            "rejecting pod";
            "container_name" => &container.name
        );
        return Err(anyhow!(
            "container {} without readiness probe is not accepted",
            &container.name
        ));
    }
    Ok(())
}

fn validate_pod(pod: &apicore::PodSpec) -> Result<()> {
    let mut err_message = String::new();
    for container in &pod.containers {
        let container_valid = validate_container(container);
        if container_valid.is_err() {
            err_message = err_message
                + &format!(
                    "container {} is invalid: {}\n",
                    container.name,
                    container_valid.unwrap_err()
                );
        }
    }
    if let Some(init_containers) = &pod.init_containers {
        for container in init_containers {
            let container_valid = validate_container(container);
            if container_valid.is_err() {
                err_message = err_message
                    + &format!(
                        "init container {} is invalid: {}\n",
                        container.name,
                        container_valid.unwrap_err()
                    );
            }
        }
    }
    if let Some(ephemeral_containers) = &pod.ephemeral_containers {
        for container in ephemeral_containers {
            let container_valid = validate_ephemeral_container(container);
            if container_valid.is_err() {
                err_message = err_message
                    + &format!(
                        "ephemeral container {} is invalid: {}\n",
                        container.name,
                        container_valid.unwrap_err()
                    );
            }
        }
    }
    if err_message.is_empty() {
        return Ok(());
    }
    Err(anyhow!(err_message))
}

fn validate(payload: &[u8]) -> CallResult {
    let validation_request: ValidationRequest<Settings> = ValidationRequest::new(payload)?;

    info!(LOG_DRAIN, "starting validation");
    match validation_request.extract_pod_spec_from_object() {
        Ok(pod_spec) => {
            if let Some(pod_spec) = pod_spec {
                return match validate_pod(&pod_spec) {
                    Ok(_) => kubewarden::accept_request(),
                    Err(err) => kubewarden::reject_request(Some(err.to_string()), None, None, None),
                };
            };
            // If there is not pod spec, just accept it. There is no data to be
            // validated.
            kubewarden::accept_request()
        }
        Err(_) => {
            warn!(LOG_DRAIN, "cannot unmarshal resource: this policy does not know how to evaluate this resource; accept it");
            kubewarden::reject_request(
                Some("Cannot parse validation request".to_string()),
                None,
                None,
                None,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use kubewarden_policy_sdk::test::Testcase;

    #[test]
    fn accept_pod_with_probes() -> Result<(), ()> {
        let request_file = "test_data/pod_creation.json";
        let tc = Testcase {
            name: String::from("Valid name"),
            fixture_file: String::from(request_file),
            expected_validation_result: true,
            settings: Settings {},
        };

        let res = tc.eval(validate).unwrap();
        assert!(
            res.mutated_object.is_none(),
            "Something mutated with test case: {}",
            tc.name,
        );

        Ok(())
    }

    #[test]
    fn reject_pod_without_liveness() -> Result<(), ()> {
        let request_file = "test_data/pod_creation_invalid_liveness.json";
        let tc = Testcase {
            name: String::from("Bad name"),
            fixture_file: String::from(request_file),
            expected_validation_result: false,
            settings: Settings {},
        };

        let res = tc.eval(validate).unwrap();
        assert!(
            res.mutated_object.is_none(),
            "Something mutated with test case: {}",
            tc.name,
        );

        Ok(())
    }

    #[test]
    fn reject_pod_without_readiness() -> Result<(), ()> {
        let request_file = "test_data/pod_creation_invalid_readiness.json";
        let tc = Testcase {
            name: String::from("Bad name"),
            fixture_file: String::from(request_file),
            expected_validation_result: false,
            settings: Settings {},
        };

        let res = tc.eval(validate).unwrap();
        assert!(
            res.mutated_object.is_none(),
            "Something mutated with test case: {}",
            tc.name,
        );

        Ok(())
    }

    #[test]
    fn accept_pod_init_containers_with_probes() -> Result<(), ()> {
        let request_file = "test_data/pod_creation_init_container.json";
        let tc = Testcase {
            name: String::from("Valid name"),
            fixture_file: String::from(request_file),
            expected_validation_result: true,
            settings: Settings {},
        };

        let res = tc.eval(validate).unwrap();
        assert!(
            res.mutated_object.is_none(),
            "Something mutated with test case: {}",
            tc.name,
        );

        Ok(())
    }

    #[test]
    fn reject_pod_init_containers_without_liveness() -> Result<(), ()> {
        let request_file = "test_data/pod_creation_invalid_liveness_init_container.json";
        let tc = Testcase {
            name: String::from("Bad name"),
            fixture_file: String::from(request_file),
            expected_validation_result: false,
            settings: Settings {},
        };

        let res = tc.eval(validate).unwrap();
        assert!(
            res.mutated_object.is_none(),
            "Something mutated with test case: {}",
            tc.name,
        );

        Ok(())
    }

    #[test]
    fn reject_pod_init_containers_without_readiess() -> Result<(), ()> {
        let request_file = "test_data/pod_creation_invalid_readiness_init_container.json";
        let tc = Testcase {
            name: String::from("Bad name"),
            fixture_file: String::from(request_file),
            expected_validation_result: false,
            settings: Settings {},
        };

        let res = tc.eval(validate).unwrap();
        assert!(
            res.mutated_object.is_none(),
            "Something mutated with test case: {}",
            tc.name,
        );

        Ok(())
    }
}
