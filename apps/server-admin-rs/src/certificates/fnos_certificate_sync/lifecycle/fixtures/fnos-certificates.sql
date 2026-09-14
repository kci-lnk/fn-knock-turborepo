-- fnOS 1.2.0604 reference structure, captured without certificate or credential data.
CREATE TABLE public.cert (
    id bigserial NOT NULL,
    domain varchar(1024) NOT NULL,
    san varchar(1028),
    valid_from bigint,
    valid_to bigint,
    encrypt_type varchar(16),
    issued_by varchar(256),
    last_renew_time bigint NOT NULL,
    des varchar(1024),
    is_default smallint,
    renewal smallint DEFAULT 0,
    source varchar(16),
    private_key varchar(256),
    certificate varchar(256),
    issuer_certificate varchar(256),
    status varchar(16),
    created_time bigint,
    updated_time bigint
);
CREATE TABLE public.cert_renew (
    id bigserial NOT NULL,
    cert_id bigint NOT NULL,
    verify_type varchar(16),
    renew_email varchar(256),
    platform varchar(16) DEFAULT 'letsencrypt'::character varying,
    ddns_provider varchar(16),
    access_key varchar(256),
    access_secret varchar(256),
    renew_cert_url varchar(1024),
    ddns_related_id bigint NOT NULL DEFAULT 0,
    last_renew_time bigint,
    last_renew_error varchar(1024),
    created_time bigint,
    updated_time bigint
);
CREATE TABLE public.cert_used_config (
    id bigserial NOT NULL,
    cert_id bigint NOT NULL,
    service_name varchar(64),
    created_time bigint,
    updated_time bigint
);
