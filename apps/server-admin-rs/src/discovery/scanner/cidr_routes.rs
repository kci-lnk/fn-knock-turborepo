use super::*;

pub(crate) async fn lookup_cidr_region(
    state: &AppState,
    province: &str,
    query_city: Option<&str>,
) -> Result<CidrRegionLookup, String> {
    let input = ScannerCidrExemptionRegionInput {
        province: province.to_string(),
        query_city: query_city.map(ToString::to_string),
    };
    let lookup = lookup_region_cidrs(state, &input)
        .await
        .map_err(|error| error.to_string())?;
    let selection = serde_json::to_value(&lookup.selection).map_err(|error| error.to_string())?;
    Ok(CidrRegionLookup {
        selection,
        cidrs: lookup.cidrs,
    })
}

pub(super) async fn get_cidr_provinces(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    cidr_response(get_cidr_provinces_payload(&state).await, &translator)
}

pub(super) async fn get_cidr_cities(
    State(state): State<AppState>,
    Query(query): Query<CidrCityQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    cidr_response(
        get_cidr_cities_payload(&state, &query.province, Some(&translator)).await,
        &translator,
    )
}

pub(super) async fn get_cidr_selector(
    State(state): State<AppState>,
    Query(query): Query<CidrProvinceQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let result = async {
        let provinces = get_cidr_provinces_payload(&state).await?;
        let province = query
            .province
            .as_deref()
            .map(normalize_string)
            .filter(|value| !value.is_empty());
        let cities = match province {
            Some(province) => get_cidr_cities_payload(&state, &province, Some(&translator)).await?,
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
    cidr_response(
        get_cidr_lookup_payload(
            &state,
            &query.province,
            query.city.as_deref(),
            Some(&translator),
        )
        .await,
        &translator,
    )
}

pub(super) fn cidr_response(
    result: Result<Value, ScannerError>,
    translator: &Translator,
) -> Response {
    match result {
        Ok(payload) => response::ok(payload).into_response(),
        Err(ScannerError::BadRequest(message)) => response::error(
            StatusCode::BAD_REQUEST,
            localize_scanner_error(translator, &message),
        ),
        Err(ScannerError::Cidr(message)) => response::error(
            StatusCode::BAD_GATEWAY,
            localize_cidr_error(translator, &message),
        ),
        Err(error) => {
            tracing::warn!(%error, "CIDR route failed");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cidr_text(translator, "serviceError"),
            )
        }
    }
}
