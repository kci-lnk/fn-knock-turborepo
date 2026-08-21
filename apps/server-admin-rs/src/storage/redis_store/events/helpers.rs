use super::*;

pub(super) fn system_event_page(
    events: impl IntoIterator<Item = Value>,
    page: i64,
    limit: i64,
    search: &str,
    event_type: Option<&str>,
    level: Option<&str>,
    source: Option<&str>,
) -> Value {
    let safe_page = page.max(1);
    let safe_limit = limit.clamp(1, 100);
    let page_start = (safe_page - 1) * safe_limit;
    let mut total = 0_i64;
    let mut page_events = Vec::new();
    for event in events {
        if !system_event_matches_filters(&event, search, event_type, level, source) {
            continue;
        }
        if total >= page_start && page_events.len() < safe_limit as usize {
            page_events.push(event);
        }
        total += 1;
    }
    json!({ "events": page_events, "total": total })
}
