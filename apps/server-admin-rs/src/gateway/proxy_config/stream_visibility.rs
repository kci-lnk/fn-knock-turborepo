use super::*;

fn visibility_inputs(visibility: Option<&Value>) -> Value {
    let mode = visibility
        .and_then(|value| value.get("mode"))
        .and_then(Value::as_str)
        .unwrap_or("inherit");
    if mode != "custom" {
        return json!({ "mode": mode });
    }
    let selections = visibility
        .and_then(|value| value.get("selections"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|selection| {
            json!({
                "province": selection.get("province"),
                "query_city": selection.get("query_city").or_else(|| selection.get("queryCity")),
                "operator": selection.get("operator"),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "mode": mode,
        "selections": selections,
        "custom_cidrs": visibility.and_then(|value| value.get("custom_cidrs")),
    })
}

pub(super) fn stream_mapping_precondition_matches(expected: &Value, previous: &Value) -> bool {
    stream_mapping_inputs(expected) == stream_mapping_inputs(previous)
        // Older clients predate the visibility field in expected_mappings.
        && expected.get("visibility").is_none_or(|visibility| {
            validate_visibility_input(visibility).is_ok()
                && visibility_inputs(Some(visibility))
                    == visibility_inputs(previous.get("visibility"))
        })
}

fn validate_visibility_input(visibility: &Value) -> Result<(), String> {
    match visibility.get("mode").and_then(Value::as_str) {
        Some("inherit" | "disabled") => return Ok(()),
        Some("custom") => {}
        _ => return Err("visibility mode must be inherit, custom or disabled".into()),
    }
    if let Some(cidrs) = visibility.get("custom_cidrs")
        && !cidrs
            .as_array()
            .is_some_and(|cidrs| cidrs.iter().all(Value::is_string))
    {
        return Err("visibility custom_cidrs must be an array of strings".into());
    }
    if let Some(selections) = visibility.get("selections")
        && !selections.as_array().is_some_and(|selections| {
            selections.iter().all(|selection| {
                selection
                    .get("province")
                    .and_then(Value::as_str)
                    .is_some_and(|province| !province.trim().is_empty())
                    && ["query_city", "queryCity"].into_iter().all(|field| {
                        selection
                            .get(field)
                            .is_none_or(|city| city.is_null() || city.is_string())
                    })
            })
        })
    {
        return Err(
            "visibility selections must each specify a province and an optional city".into(),
        );
    }
    Ok(())
}

pub(super) async fn prepare_stream_mapping_visibility(
    state: &AppState,
    previous: &[Value],
    next: &mut [Value],
    config: &mut Value,
) -> Result<(), String> {
    let previous_by_key = previous
        .iter()
        .filter_map(|mapping| mapping_key(mapping).map(|key| (key, mapping)))
        .collect::<HashMap<_, _>>();
    for mapping in next {
        let Some(key) = mapping_key(mapping) else {
            continue;
        };
        let object = ensure_object(mapping);
        let requested = object.remove("visibility");
        let current = previous_by_key
            .get(&key)
            .and_then(|mapping| mapping.get("visibility"));
        let Some(requested) = requested else {
            if let Some(current) = current {
                object.insert("visibility".into(), current.clone());
            }
            continue;
        };
        validate_visibility_input(&requested)?;
        let normalized = normalize_host_mapping_visibility(Some(&requested), current, false)?;
        match normalized["mode"].as_str() {
            Some("custom") => {
                let inputs = visibility_inputs(Some(&normalized));
                let has_policy = |visibility: &Value| {
                    let Some(id) = visibility.get("policy_id").and_then(Value::as_str) else {
                        return false;
                    };
                    config
                        .get("stream_access_policies")
                        .and_then(Value::as_array)
                        .is_some_and(|policies| {
                            policies
                                .iter()
                                .any(|policy| policy.get("id").and_then(Value::as_str) == Some(id))
                        })
                };
                let existing = current
                    .filter(|visibility| {
                        visibility_inputs(Some(visibility)) == inputs && has_policy(visibility)
                    })
                    .or_else(|| {
                        // A change of listener identity may carry the same compiled policy.
                        previous
                            .iter()
                            .filter_map(|mapping| mapping.get("visibility"))
                            .find(|visibility| {
                                visibility_inputs(Some(visibility)) == inputs
                                    && has_policy(visibility)
                            })
                    });
                let visibility = if let Some(existing) = existing {
                    existing.clone()
                } else {
                    let compiled = gateway_settings::compile_host_visibility_config(
                        state,
                        normalized
                            .as_object()
                            .ok_or("visibility must be an object")?,
                    )
                    .await?;
                    let policies = ensure_object(config)
                        .entry("stream_access_policies")
                        .or_insert_with(|| json!([]));
                    policies
                        .as_array_mut()
                        .ok_or("stream_access_policies must be an array")?
                        .push(compiled.policy.to_transport_value());
                    compiled.config
                };
                object.insert("visibility".into(), visibility);
            }
            Some("disabled") => {
                object.insert(
                    "visibility".into(),
                    json!({ "mode": "disabled", "selections": [], "custom_cidrs": [] }),
                );
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_visibility_distinguishes_city_alias_from_province_wide_scope() {
        let rule = |selection| {
            json!({
                "visibility": {
                    "mode": "custom",
                    "selections": [selection],
                    "custom_cidrs": [],
                }
            })
        };
        let city = rule(json!({"province": "浙江", "queryCity": "杭州"}));
        let canonical = rule(json!({"province": "浙江", "query_city": "杭州"}));
        let province = rule(json!({"province": "浙江", "query_city": null}));
        assert!(stream_mapping_precondition_matches(&city, &canonical));
        assert!(!stream_mapping_precondition_matches(&city, &province));
    }

    #[test]
    fn stream_visibility_rejects_partial_and_malformed_explicit_policies() {
        for invalid in [
            json!({}),
            json!({"mode": "custm"}),
            json!({"mode": "custom", "custom_cidrs": [42, "192.0.2.0/24"]}),
            json!({"mode": "custom", "selections": [{"province": "浙江", "query_city": 42}]}),
            json!({"mode": "custom", "selections": [{"query_city": "杭州"}]}),
        ] {
            assert!(validate_visibility_input(&invalid).is_err(), "{invalid}");
            assert!(!stream_mapping_precondition_matches(
                &json!({"visibility": invalid}),
                &json!({}),
            ));
        }
        assert!(stream_mapping_precondition_matches(&json!({}), &json!({})));
    }
}
