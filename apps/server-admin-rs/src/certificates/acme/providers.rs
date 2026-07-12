use super::*;

pub(super) fn windows_acme_dns_providers(t: &Translator) -> Vec<Value> {
    let common_label = t.t("server.acmeDnsProviders.groups.common");
    let domestic_label = t.t("server.acmeDnsProviders.groups.domestic");
    let international_label = t.t("server.acmeDnsProviders.groups.international");
    let common = common_label.as_str();
    let domestic = domestic_label.as_str();
    let international = international_label.as_str();
    let mut providers = vec![
        simple_provider(
            "dns_ali",
            &t.t("server.acmeDnsProviders.labels.aliyun"),
            common,
            &["Ali_Key", "Ali_Secret", "Ali_Domain"],
            &["Ali_Domain"],
        ),
        simple_provider(
            "dns_baiducloud",
            "Baidu Cloud DNS",
            domestic,
            &[
                "BAIDU_ACCESS_KEY_ID",
                "BAIDU_SECRET_ACCESS_KEY",
                "root_domain",
            ],
            &["root_domain"],
        ),
        json!({
            "dnsType": "dns_cf",
            "label": "Cloudflare",
            "group": common,
            "credentialSchemes": [
                scheme("api-token", "API Token", &["CF_Token", "CF_Zone_ID", "CF_Account_ID"], &["CF_Zone_ID", "CF_Account_ID"]),
                scheme("global-key", "Global API Key", &["CF_Key", "CF_Email", "CF_Zone_ID", "CF_Account_ID"], &["CF_Zone_ID", "CF_Account_ID"]),
            ],
        }),
        simple_provider(
            "dns_dp",
            "DNSPod",
            common,
            &["DP_Id", "DP_Key", "DP_Domain"],
            &["DP_Domain"],
        ),
        simple_provider(
            "dns_tencent",
            &t.t("server.acmeDnsProviders.labels.tencentCloudDnspod"),
            common,
            &["Tencent_SecretId", "Tencent_SecretKey"],
            &[],
        ),
        simple_provider("dns_duckdns", "DuckDNS", common, &["DuckDNS_Token"], &[]),
        simple_provider(
            "dns_dynu",
            "Dynu",
            international,
            &["Dynu_ClientId", "Dynu_Secret"],
            &[],
        ),
        simple_provider("dns_dynv6", "dynv6", international, &["DYNV6_TOKEN"], &[]),
        simple_provider(
            "dns_gd",
            "GoDaddy",
            international,
            &["GD_Key", "GD_Secret", "GD_Domain"],
            &["GD_Domain"],
        ),
        simple_provider(
            "dns_huaweicloud",
            &t.t("server.acmeDnsProviders.labels.huaweiCloudDns"),
            domestic,
            &[
                "HUAWEICLOUD_Username",
                "HUAWEICLOUD_Password",
                "HUAWEICLOUD_DomainName",
                "HUAWEICLOUD_Region",
                "HUAWEICLOUD_ProjectName",
            ],
            &["HUAWEICLOUD_Region", "HUAWEICLOUD_ProjectName"],
        ),
        simple_provider(
            "dns_porkbun",
            "Porkbun",
            international,
            &[
                "PORKBUN_API_KEY",
                "PORKBUN_SECRET_API_KEY",
                "PORKBUN_DOMAIN",
            ],
            &["PORKBUN_DOMAIN"],
        ),
    ];
    let default_credential_label = t.t("server.acmeDnsProviders.credentialSchemes.default");
    localize_default_credential_labels(&mut providers, &default_credential_label);
    providers
}

pub(super) fn acme_dns_providers(t: &Translator) -> Vec<Value> {
    let common_label = t.t("server.acmeDnsProviders.groups.common");
    let domestic_label = t.t("server.acmeDnsProviders.groups.domestic");
    let international_label = t.t("server.acmeDnsProviders.groups.international");
    let self_hosted_label = t.t("server.acmeDnsProviders.groups.selfHostedAdvanced");
    let common = common_label.as_str();
    let domestic = domestic_label.as_str();
    let international = international_label.as_str();
    let self_hosted = self_hosted_label.as_str();
    let default_credential_label = t.t("server.acmeDnsProviders.credentialSchemes.default");
    let mut providers = vec![
        json!({
            "dnsType": "dns_cf",
            "label": "Cloudflare",
            "group": common,
            "credentialSchemes": [
                scheme("global-key", "Global API Key", &["CF_Key", "CF_Email"], &[]),
                scheme("api-token", "API Token", &["CF_Token", "CF_Zone_ID", "CF_Account_ID"], &["CF_Zone_ID", "CF_Account_ID"]),
            ],
        }),
        simple_provider(
            "dns_ali",
            &t.t("server.acmeDnsProviders.labels.aliyun"),
            common,
            &["Ali_Key", "Ali_Secret"],
            &[],
        ),
        simple_provider("dns_dp", "DNSPod", common, &["DP_Id", "DP_Key"], &[]),
        simple_provider(
            "dns_tencent",
            &t.t("server.acmeDnsProviders.labels.tencentCloudDnspod"),
            common,
            &["Tencent_SecretId", "Tencent_SecretKey"],
            &[],
        ),
        simple_provider("dns_duckdns", "DuckDNS", common, &["DuckDNS_Token"], &[]),
        simple_provider("dns_gd", "GoDaddy", common, &["GD_Key", "GD_Secret"], &[]),
        simple_provider("dns_dgon", "DigitalOcean", common, &["DO_API_KEY"], &[]),
        simple_provider(
            "dns_netlify",
            "Netlify",
            common,
            &["NETLIFY_ACCESS_TOKEN"],
            &[],
        ),
        simple_provider("dns_vercel", "Vercel", common, &["VERCEL_TOKEN"], &[]),
        simple_provider(
            "dns_aws",
            "AWS Route53",
            common,
            &["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"],
            &[],
        ),
        simple_provider(
            "dns_gcloud",
            "Google Cloud DNS (gcloud)",
            common,
            &["CLOUDSDK_ACTIVE_CONFIG_NAME"],
            &["CLOUDSDK_ACTIVE_CONFIG_NAME"],
        ),
        json!({
            "dnsType": "dns_azure",
            "label": "Azure DNS",
            "group": common,
            "credentialSchemes": [
                scheme("service-principal", "Service Principal", &["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_TENANTID", "AZUREDNS_APPID", "AZUREDNS_CLIENTSECRET"], &[]),
                scheme("bearer-token", "Bearer Token", &["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_BEARERTOKEN"], &[]),
                scheme("managed-identity", "Managed Identity", &["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_MANAGEDIDENTITY"], &[]),
            ],
        }),
        simple_provider(
            "dns_porkbun",
            "Porkbun",
            common,
            &["PORKBUN_API_KEY", "PORKBUN_SECRET_API_KEY"],
            &[],
        ),
        json!({
            "dnsType": "dns_dynv6",
            "label": "dynv6",
            "group": common,
            "credentialSchemes": [
                scheme("rest-token", "REST API Token", &["DYNV6_TOKEN"], &[]),
                scheme("ssh-key", "SSH Key", &["KEY"], &[]),
            ],
        }),
        simple_provider(
            "dns_huaweicloud",
            &t.t("server.acmeDnsProviders.labels.huaweiCloudDns"),
            domestic,
            &[
                "HUAWEICLOUD_Username",
                "HUAWEICLOUD_Password",
                "HUAWEICLOUD_DomainName",
            ],
            &[],
        ),
        simple_provider(
            "dns_jd",
            &t.t("server.acmeDnsProviders.labels.jdCloudDns"),
            domestic,
            &["JD_ACCESS_KEY_ID", "JD_ACCESS_KEY_SECRET", "JD_REGION"],
            &[],
        ),
        simple_provider("dns_la", "DNS.LA", domestic, &["LA_Id", "LA_Sk"], &[]),
        simple_provider(
            "dns_west_cn",
            &t.t("server.acmeDnsProviders.labels.westCn"),
            domestic,
            &["WEST_Username", "WEST_Key"],
            &[],
        ),
        simple_provider(
            "dns_linode_v4",
            "Linode",
            international,
            &["LINODE_V4_API_KEY"],
            &[],
        ),
        simple_provider("dns_vultr", "Vultr", international, &["VULTR_API_KEY"], &[]),
        simple_provider(
            "dns_ovh",
            "OVH",
            international,
            &["OVH_AK", "OVH_AS", "OVH_CK", "OVH_END_POINT"],
            &["OVH_END_POINT"],
        ),
        simple_provider(
            "dns_hetzner",
            "Hetzner",
            international,
            &["HETZNER_Token"],
            &[],
        ),
        simple_provider(
            "dns_namecheap",
            "Namecheap",
            international,
            &[
                "NAMECHEAP_API_KEY",
                "NAMECHEAP_USERNAME",
                "NAMECHEAP_SOURCEIP",
            ],
            &[],
        ),
        simple_provider(
            "dns_namecom",
            "Name.com",
            international,
            &["Namecom_Username", "Namecom_Token"],
            &[],
        ),
        simple_provider(
            "dns_namesilo",
            "NameSilo",
            international,
            &["Namesilo_Key"],
            &[],
        ),
        simple_provider(
            "dns_dreamhost",
            "DreamHost",
            international,
            &["DH_API_KEY"],
            &[],
        ),
        simple_provider(
            "dns_freedns",
            "FreeDNS",
            international,
            &["FREEDNS_User", "FREEDNS_Password"],
            &[],
        ),
        simple_provider(
            "dns_dyn",
            "Dyn Managed DNS",
            international,
            &["DYN_Customer", "DYN_Username", "DYN_Password"],
            &[],
        ),
        simple_provider(
            "dns_dynu",
            "Dynu",
            international,
            &["Dynu_ClientId", "Dynu_Secret"],
            &[],
        ),
        simple_provider(
            "dns_bunny",
            "Bunny DNS",
            international,
            &["BUNNY_API_KEY"],
            &[],
        ),
        simple_provider("dns_desec", "deSEC", international, &["DEDYN_TOKEN"], &[]),
        simple_provider(
            "dns_freemyip",
            "FreeMyIP",
            international,
            &["FREEMYIP_Token"],
            &[],
        ),
        simple_provider(
            "dns_ipv64",
            "IPv64.net",
            international,
            &["IPv64_Token"],
            &[],
        ),
        simple_provider(
            "dns_scaleway",
            "Scaleway",
            international,
            &["SCALEWAY_API_TOKEN"],
            &[],
        ),
        simple_provider(
            "dns_easydns",
            "easyDNS",
            international,
            &["EASYDNS_Token", "EASYDNS_Key"],
            &[],
        ),
        simple_provider(
            "dns_zoneedit",
            "ZoneEdit",
            international,
            &["ZONEEDIT_ID", "ZONEEDIT_Token"],
            &[],
        ),
        simple_provider("dns_zonomi", "Zonomi", international, &["ZM_Key"], &[]),
        simple_provider(
            "dns_dnsexit",
            "DNSExit",
            international,
            &["DNSEXIT_API_KEY", "DNSEXIT_AUTH_USER", "DNSEXIT_AUTH_PASS"],
            &[],
        ),
        json!({
            "dnsType": "dns_yandex360",
            "label": "Yandex 360",
            "group": international,
            "credentialSchemes": [
                scheme("oauth-client", "OAuth Client", &["YANDEX360_CLIENT_ID", "YANDEX360_CLIENT_SECRET", "YANDEX360_ORG_ID"], &["YANDEX360_ORG_ID"]),
                scheme("access-token", "Access Token", &["YANDEX360_ACCESS_TOKEN", "YANDEX360_ORG_ID"], &["YANDEX360_ORG_ID"]),
            ],
        }),
        simple_provider(
            "dns_mydnsjp",
            "MyDNS.JP",
            international,
            &["MYDNSJP_MasterID", "MYDNSJP_Password"],
            &[],
        ),
        simple_provider(
            "dns_gandi_livedns",
            "Gandi LiveDNS",
            international,
            &["GANDI_LIVEDNS_KEY"],
            &[],
        ),
        simple_provider("dns_nsone", "NS1", international, &["NS1_Key"], &[]),
        simple_provider(
            "dns_dnsimple",
            "DNSimple",
            international,
            &["DNSimple_OAUTH_TOKEN"],
            &[],
        ),
        json!({
            "dnsType": "dns_cloudns",
            "label": "ClouDNS",
            "group": international,
            "credentialSchemes": [
                scheme("auth-id", "Auth ID", &["CLOUDNS_AUTH_ID", "CLOUDNS_AUTH_PASSWORD"], &[]),
                scheme("sub-auth-id", "Sub Auth ID", &["CLOUDNS_SUB_AUTH_ID", "CLOUDNS_AUTH_PASSWORD"], &[]),
            ],
        }),
        simple_provider(
            "dns_he",
            "Hurricane Electric",
            international,
            &["HE_Username", "HE_Password"],
            &[],
        ),
        simple_provider(
            "dns_transip",
            "TransIP",
            international,
            &["TRANSIP_Username", "TRANSIP_Key_File"],
            &[],
        ),
        simple_provider(
            "dns_doapi",
            "Domain-Offensive",
            international,
            &["DO_LETOKEN"],
            &[],
        ),
        simple_provider(
            "dns_acmedns",
            "acme-dns",
            self_hosted,
            &[
                "ACMEDNS_USERNAME",
                "ACMEDNS_PASSWORD",
                "ACMEDNS_SUBDOMAIN",
                "ACMEDNS_BASE_URL",
            ],
            &["ACMEDNS_BASE_URL"],
        ),
        simple_provider(
            "dns_nsupdate",
            "nsupdate",
            self_hosted,
            &[
                "NSUPDATE_SERVER",
                "NSUPDATE_SERVER_PORT",
                "NSUPDATE_KEY",
                "NSUPDATE_ZONE",
            ],
            &["NSUPDATE_SERVER_PORT", "NSUPDATE_KEY", "NSUPDATE_ZONE"],
        ),
        simple_provider(
            "dns_pdns",
            "PowerDNS",
            self_hosted,
            &["PDNS_Url", "PDNS_ServerId", "PDNS_Token", "PDNS_Ttl"],
            &["PDNS_Ttl"],
        ),
        simple_provider(
            "dns_technitium",
            "Technitium DNS",
            self_hosted,
            &[
                "Technitium_Server",
                "Technitium_Token",
                "Technitium_Expiry_Ttl",
            ],
            &["Technitium_Expiry_Ttl"],
        ),
        simple_provider(
            "dns_pleskxml",
            "Plesk XML API",
            self_hosted,
            &["pleskxml_uri", "pleskxml_user", "pleskxml_pass"],
            &[],
        ),
        simple_provider(
            "dns_cpanel",
            "cPanel",
            self_hosted,
            &["cPanel_Username", "cPanel_Apitoken", "cPanel_Hostname"],
            &[],
        ),
        simple_provider(
            "dns_da",
            "DirectAdmin",
            self_hosted,
            &["DA_Api", "DA_Api_Insecure"],
            &[],
        ),
        simple_provider(
            "dns_ispconfig",
            "ISPConfig",
            self_hosted,
            &[
                "ISPC_User",
                "ISPC_Password",
                "ISPC_Api",
                "ISPC_Api_Insecure",
            ],
            &[],
        ),
        simple_provider(
            "dns_opnsense",
            "OPNsense",
            self_hosted,
            &[
                "OPNs_Host",
                "OPNs_Port",
                "OPNs_Key",
                "OPNs_Token",
                "OPNs_Api_Insecure",
            ],
            &["OPNs_Port", "OPNs_Api_Insecure"],
        ),
    ];
    localize_default_credential_labels(&mut providers, &default_credential_label);
    providers
}

pub(super) fn localize_default_credential_labels(providers: &mut [Value], label: &str) {
    for provider in providers {
        let Some(schemes) = provider
            .get_mut("credentialSchemes")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for scheme in schemes {
            if scheme.get("id").and_then(Value::as_str) == Some("default") {
                scheme["label"] = json!(label);
            }
        }
    }
}

pub(super) fn simple_provider(
    dns_type: &str,
    label: &str,
    group: &str,
    fields: &[&str],
    optional_fields: &[&str],
) -> Value {
    json!({
        "dnsType": dns_type,
        "label": label,
        "group": group,
        "credentialSchemes": [scheme("default", "Default credentials", fields, optional_fields)],
    })
}

pub(super) fn scheme(id: &str, label: &str, fields: &[&str], optional_fields: &[&str]) -> Value {
    let optional = optional_fields.iter().copied().collect::<BTreeSet<_>>();
    json!({
        "id": id,
        "label": label,
        "fields": fields.iter().map(|key| {
            json!({
                "key": key,
                "required": !optional.contains(key),
            })
        }).collect::<Vec<_>>(),
    })
}
