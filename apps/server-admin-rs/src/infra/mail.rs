//! Transport-only mail infrastructure. Callers own configuration and retry policy.
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Attachment, Mailbox, MultiPart, SinglePart, header::ContentType},
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        client::Tls,
        response::Response,
    },
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use utoipa::ToSchema;

#[derive(Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub security: String,
    pub auth_mode: String,
    pub username: String,
    pub timeout_seconds: u64,
}
impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 465,
            security: "ssl_tls".into(),
            auth_mode: "auto".into(),
            username: String::new(),
            timeout_seconds: 30,
        }
    }
}
#[derive(Debug)]
pub(crate) struct MailError {
    pub code: &'static str,
    pub retryable: bool,
}
impl std::fmt::Display for MailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}
impl std::error::Error for MailError {}
impl MailError {
    pub fn permanent(code: &'static str) -> Self {
        Self {
            code,
            retryable: false,
        }
    }
}
pub(crate) struct MailAttachment {
    pub filename: String,
    pub bytes: Vec<u8>,
}
pub(crate) struct MailMessage {
    pub from: Mailbox,
    pub to: Vec<Mailbox>,
    pub subject: String,
    pub body: String,
    pub message_id: String,
    pub attachment: Option<MailAttachment>,
}
impl MailMessage {
    pub fn build(self) -> Result<Message, MailError> {
        if self.to.is_empty() {
            return Err(MailError::permanent("recipient_required"));
        }
        let mut builder = Message::builder()
            .from(self.from)
            .subject(self.subject)
            .message_id(Some(self.message_id));
        for address in self.to {
            builder = builder.to(address);
        }
        let body = SinglePart::plain(self.body);
        let result = if let Some(attachment) = self.attachment {
            builder.multipart(
                MultiPart::mixed().singlepart(body).singlepart(
                    Attachment::new(attachment.filename).body(
                        attachment.bytes,
                        ContentType::parse("application/octet-stream")
                            .map_err(|_| MailError::permanent("invalid_content_type"))?,
                    ),
                ),
            )
        } else {
            builder.singlepart(body)
        };
        result.map_err(|_| MailError::permanent("invalid_message"))
    }
}
pub(crate) fn transport(
    config: &SmtpConfig,
    password: &str,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, MailError> {
    let mut builder = match config.security.as_str() {
        "none" => {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host).tls(Tls::None)
        }
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(|_| MailError::permanent("invalid_smtp_host"))?,
        "ssl_tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .map_err(|_| MailError::permanent("invalid_smtp_host"))?,
        _ => return Err(MailError::permanent("invalid_tls_mode")),
    }
    .port(config.port);
    if config.auth_mode != "none" && !config.username.trim().is_empty() {
        builder = builder.credentials(Credentials::new(config.username.clone(), password.into()));
        builder = match config.auth_mode.as_str() {
            "plain" => builder.authentication(vec![Mechanism::Plain]),
            "login" => builder.authentication(vec![Mechanism::Login]),
            _ => builder,
        };
    }
    Ok(builder.build())
}
pub(crate) async fn send(
    config: &SmtpConfig,
    password: &str,
    message: Message,
) -> Result<Response, MailError> {
    let mailer = transport(config, password)?;
    match tokio::time::timeout(
        Duration::from_secs(config.timeout_seconds.clamp(1, 120)),
        mailer.send(message),
    )
    .await
    {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(error)) => Err(MailError {
            code: if error.is_permanent() {
                "smtp_rejected"
            } else {
                "smtp_unavailable"
            },
            retryable: !error.is_permanent() && !error.is_client(),
        }),
        Err(_) => Err(MailError {
            code: "smtp_timeout",
            retryable: true,
        }),
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
    };
    pub async fn smtp(reply: &'static str) -> (u16, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            tokio::time::timeout(std::time::Duration::from_secs(10), async move {
                let (stream, _) = listener.accept().await.unwrap();
                let (read, mut write) = stream.into_split();
                write
                    .write_all(b"220 localhost test SMTP\r\n")
                    .await
                    .unwrap();
                let mut lines = BufReader::new(read).lines();
                let mut data = false;
                let mut captured = String::new();
                while let Some(line) = lines.next_line().await.unwrap() {
                    if data {
                        if line == "." {
                            write.write_all(b"250 queued\r\n").await.unwrap();
                            data = false;
                        } else {
                            captured.push_str(&line);
                            captured.push_str("\r\n");
                        }
                    } else if line.starts_with("EHLO") {
                        write.write_all(b"250 localhost\r\n").await.unwrap();
                    } else if line.starts_with("RCPT") {
                        write.write_all(reply.as_bytes()).await.unwrap();
                    } else if line == "DATA" {
                        write.write_all(b"354 send data\r\n").await.unwrap();
                        data = true;
                    } else if line == "QUIT" {
                        let _ = write.write_all(b"221 goodbye\r\n").await;
                        break;
                    } else {
                        write.write_all(b"250 OK\r\n").await.unwrap();
                    }
                }
                captured
            })
            .await
            .unwrap()
        });
        (port, task)
    }
}
