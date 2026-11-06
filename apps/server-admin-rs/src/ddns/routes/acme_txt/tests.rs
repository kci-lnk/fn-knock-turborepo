use super::*;
use axum::{
    body::Bytes,
    extract::OriginalUri,
    http::{HeaderMap, Method},
    routing::any,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Mock {
    kind: &'static str,
    values: Arc<Mutex<Vec<(u64, String)>>>,
    deny: bool,
}
impl Mock {
    fn list(&self) -> Value {
        let rows:Vec<_>=self.values.lock().unwrap().iter().map(|(id,value)|match self.kind{
            "dns_cf"=>json!({"id":id.to_string(),"type":"TXT","name":"_acme-challenge.example.test","content":format!("\"{value}\"")}),
            "dns_dp"=>json!({"id":id.to_string(),"type":"TXT","name":"_acme-challenge","value":value}),
            "dns_ali"=>json!({"RecordId":id.to_string(),"Type":"TXT","RR":"_acme-challenge","Value":value}),
            _=>json!({"RecordId":id,"Type":"TXT","Name":"_acme-challenge","Value":value}),
        }).collect();
        match self.kind {
            "dns_cf" => json!({"success":true,"result":rows}),
            "dns_dp" => json!({"status":{"code":"1"},"records":rows}),
            "dns_ali" => json!({"DomainRecords":{"Record":rows}}),
            _ => json!({"Response":{"RecordList":rows}}),
        }
    }
}
async fn serve(
    State(mock): State<Mock>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, Json<Value>) {
    if mock.deny {
        return (StatusCode::FORBIDDEN, Json(json!({"success":false})));
    }
    let form: HashMap<String, String> = url::form_urlencoded::parse(&body).into_owned().collect();
    let action = headers
        .get("X-TC-Action")
        .and_then(|v| v.to_str().ok())
        .or_else(|| form.get("Action").map(String::as_str))
        .unwrap_or_default();
    let data = serde_json::from_slice::<Value>(&body).unwrap_or_default();
    let id = match mock.kind {
        "dns_cf" if method == Method::DELETE => uri
            .path()
            .rsplit('/')
            .next()
            .and_then(|s| s.parse::<u64>().ok()),
        "dns_dp" if uri.path() == "/Record.Remove" => form["record_id"].parse().ok(),
        "dns_ali" if action == "DeleteDomainRecord" => form["RecordId"].parse().ok(),
        "dns_tencent" if action == "DeleteRecord" => data["RecordId"].as_u64(),
        _ => None,
    };
    let result = if let Some(id) = id {
        mock.values.lock().unwrap().retain(|(key, _)| *key != id);
        match mock.kind {
            "dns_cf" => json!({"success":true}),
            "dns_dp" => json!({"status":{"code":"1"}}),
            "dns_ali" => json!({"RecordId":id.to_string()}),
            _ => json!({"Response":{"RequestId":"request"}}),
        }
    } else if uri.path().ends_with("/zones/zone") {
        json!({"success":true,"result":{"name":"example.test","id":"zone"}})
    } else if uri.path().ends_with("/dns_records/2") {
        json!({"success":false,"errors":[{"code":81044}]})
    } else if action == "DescribeDomainRecordInfo" {
        json!({"Code":"InvalidRecordId.NotFound"})
    } else if uri.path() == "/Record.Info" {
        json!({"status":{"code":"8"}})
    } else if action == "DescribeRecord" {
        json!({"Response":{"Error":{"Code":"ResourceNotFound.NoDataOfRecord"}}})
    } else {
        mock.list()
    };
    (StatusCode::OK, Json(result))
}

async fn scenario(kind: &'static str, deny: bool) {
    let mock = Mock {
        kind,
        values: Arc::new(Mutex::new(vec![
            (1, "first".into()),
            (2, "selected".into()),
            (3, "last".into()),
        ])),
        deny,
    };
    let router = Router::new().fallback(any(serve)).with_state(mock.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    TEST_PROVIDER_ENDPOINT.scope(addr,async {
        let t=Translator::new("en");
        let creds=json!({"CF_Token":"test","CF_Zone_ID":"zone","DP_Id":"test","DP_Key":"test","Ali_Key":"test","Ali_Secret":"test","Tencent_SecretId":"test","Tencent_SecretKey":"test"});
        let provider=TxtProvider::new(&t,kind,&creds).unwrap();
        let record=TxtRecord{id:"2".into(),zone:"example.test".into(),name:"_acme-challenge.example.test".into(),value:"selected".into()};
        if deny {
            assert!(provider.delete_selected(&record,true).await.is_err());
            assert_eq!(mock.values.lock().unwrap().len(),3);
        }else{
            assert_eq!(provider.delete_selected(&record,false).await.unwrap(),"changed");
            assert_eq!(mock.values.lock().unwrap().len(),3);
            mock.values.lock().unwrap()[1].1="changed".into();
            assert_eq!(provider.delete_selected(&record,true).await.unwrap(),"changed");
            mock.values.lock().unwrap()[1].1="selected".into();
            assert_eq!(provider.delete_selected(&record,true).await.unwrap(),"deleted");
            assert_eq!(provider.delete_selected(&record,true).await.unwrap(),"absent");
            assert_eq!(*mock.values.lock().unwrap(),vec![(1,"first".into()),(3,"last".into())]);
        }
    }).await;
    server.abort();
}

#[tokio::test]
async fn four_providers_delete_only_the_selected_id() {
    for kind in ["dns_cf", "dns_dp", "dns_ali", "dns_tencent"] {
        scenario(kind, false).await;
        scenario(kind, true).await;
    }
}

#[test]
fn native_hooks_reject_fuzzy_matches_and_duplicate_values() {
    let zone = "example.test";
    let name = "_acme-challenge.example.test";
    let owner = "_acme-challenge";
    for provider in ["dns_cf", "dns_dp", "dns_tencent", "dns_ali"] {
        let rows = |entries: Vec<Value>| match provider {
            "dns_cf" => json!({"success":true,"result":entries}),
            "dns_dp" => json!({"status":{"code":"1"},"records":entries}),
            "dns_tencent" => json!({"Response":{"RecordList":entries}}),
            _ => json!({"DomainRecords":{"Record":entries}}),
        };
        let row = |relative: &str, kind: &str, value: &str, id: &str| match provider {
            "dns_cf" => {
                json!({"name":format!("{relative}.{zone}"),"type":kind,"content":format!("\"{value}\""),"id":id})
            }
            "dns_dp" => json!({"name":relative,"type":kind,"value":value,"id":id}),
            "dns_tencent" => json!({"Name":relative,"Type":kind,"Value":value,"RecordId":id}),
            _ => json!({"RR":relative,"Type":kind,"Value":value,"RecordId":id}),
        };
        let mut entries = vec![
            row("_acme-challenge.other", "TXT", "Token", "1"),
            row(owner, "TXT", "Token-longer", "2"),
            row(owner, "TXT", "token", "3"),
            row(owner, "CNAME", "Token", "4"),
        ];
        assert_eq!(
            native_cleanup_id(provider, zone, name, "Token", &rows(entries.clone())).unwrap(),
            None
        );
        entries.push(row(owner, "TXT", "Token", "5"));
        assert_eq!(
            native_cleanup_id(provider, zone, name, "Token", &rows(entries.clone())).unwrap(),
            Some("5".into())
        );
        entries.push(row(owner, "TXT", "Token", "6"));
        assert!(native_cleanup_id(provider, zone, name, "Token", &rows(entries)).is_err());
    }
}

#[test]
fn cloudflare_dns_escapes_cannot_be_mistaken_for_a_matching_json_string() {
    let name = "_acme-challenge.example.test";
    let list = json!({"success":true,"result":[{"name":name,"type":"TXT","content":r#""\u0050roof""#,"id":"unrelated"}]});
    assert!(native_cleanup_id("dns_cf", "example.test", name, "Proof", &list).is_err());
}
