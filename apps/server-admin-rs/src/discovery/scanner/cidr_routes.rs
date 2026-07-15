use super::*;

pub(super) async fn get_cidr_capabilities(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match crate::cidr::probe_configured_capabilities(&state).await {
        Ok(capabilities) => response::ok(capabilities).into_response(),
        Err(message) => response::error(
            StatusCode::BAD_GATEWAY,
            localize_cidr_error(&translator, &message),
        ),
    }
}

pub(super) async fn get_cidr_provinces(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    cidr_response(crate::cidr::provinces_payload(&state).await, &translator)
}

pub(super) async fn get_cidr_cities(
    State(state): State<AppState>,
    Query(query): Query<CidrCityQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    cidr_response(
        crate::cidr::cities_payload(&state, &query.province, Some(&translator)).await,
        &translator,
    )
}

pub(super) async fn get_cidr_selector(
    State(state): State<AppState>,
    Query(query): Query<CidrProvinceQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let result = async {
        let provinces = crate::cidr::provinces_payload(&state).await?;
        let province = query
            .province
            .as_deref()
            .map(normalize_string)
            .filter(|value| !value.is_empty());
        let cities = match province {
            Some(province) => {
                crate::cidr::cities_payload(&state, &province, Some(&translator)).await?
            }
            None => Value::Null,
        };
        Ok(json!({ "provinces": provinces, "cities": cities }))
    }
    .await;
    cidr_response(result, &translator)
}

pub(super) async fn get_cidr_cidrs(
    State(state): State<AppState>,
    Query(query): Query<CidrCityQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let result = CidrOperator::parse_optional(query.operator.as_deref())
        .map_err(crate::cidr::CidrError::BadRequest);
    let result = match result {
        Ok(operator) => {
            let query = CidrRegionQuery::new(query.province, query.city, operator);
            crate::cidr::lookup_payload(&state, &query, Some(&translator)).await
        }
        Err(error) => Err(error),
    };
    cidr_response(result, &translator)
}

fn cidr_response(
    result: Result<Value, crate::cidr::CidrError>,
    translator: &Translator,
) -> Response {
    match result {
        Ok(payload) => response::ok(payload).into_response(),
        Err(crate::cidr::CidrError::BadRequest(message)) => response::error(
            StatusCode::BAD_REQUEST,
            localize_scanner_error(translator, &message),
        ),
        Err(crate::cidr::CidrError::Service(message)) => response::error(
            if message == "CIDR operator filtering is unsupported" {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_GATEWAY
            },
            localize_cidr_error(translator, &message),
        ),
        Err(crate::cidr::CidrError::Storage(error)) => {
            tracing::warn!(%error, "CIDR route failed");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cidr_text(translator, "serviceError"),
            )
        }
    }
}
