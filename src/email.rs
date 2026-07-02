use crate::utils::{ko, ok};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use serde::{Deserialize, Serialize};
use std::fs::create_dir_all;
use std::io::Error;

#[derive(Serialize, Deserialize, Debug)]
pub struct SmtpConfig {
    pub from: String,
    pub username: String,
    pub password: String,
    pub transport: String,
    pub port: u16,
}
pub fn send(to: &str, subject: &str, message: &str) -> Result<(), Error> {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    let path = format!("{home}/.config/lys");
    create_dir_all(path.as_str())?;
    let config_path = format!("{path}/smtp_config.yml");
    let config: SmtpConfig =
        serde_yaml::from_reader(std::fs::File::open(&config_path)?).expect("msg");
    let from = config.from;
    let smtp_username = config.username;
    let smtp_password = config.password;
    let smtp_transport = config.transport;
    let port = config.port;
    // 1. Construction de l'email
    let email = Message::builder()
        .from(from.as_str().parse().expect("failed to parse from"))
        .to(to.parse().expect("failed to parse to"))
        .subject(subject)
        .body(String::from(message))
        .unwrap();

    // 2. Configuration des identifiants SMTP
    let creds = Credentials::new(smtp_username.to_owned(), smtp_password.to_owned());

    // 3. Configuration du relais (ex: Mailjet, Sendgrid, ou un serveur interne)
    // Remplacer "smtp.fournisseur.com" par le bon domaine
    let mailer = SmtpTransport::starttls_relay(&smtp_transport)
        .unwrap()
        .port(port)
        .credentials(creds)
        .build();

    // 4. Envoi et gestion du résultat
    match mailer.send(&email) {
        Ok(_) => {
            ok("Email send successfully");
            Ok(())
        }
        Err(_) => {
            Err(Error::other("Failed to send email"))
        }
    }
}

pub fn edit_smtp_config() -> Result<(), Error> {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    let path = format!("{home}/.config/lys");
    create_dir_all(path.as_str())?;
    let config_path = format!("{path}/smtp_config.yml");

    if !std::path::Path::new(&config_path).exists() {
        ko("SMTP configuration file does not exist. Please create it first.");
        return Err(Error::other(
            "SMTP configuration file does not exist. Please create it first.",
        ));
    }

    // Open the configuration file in the default editor
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let status = std::process::Command::new(editor)
        .arg(&config_path)
        .status()
        .expect("Failed to open editor");

    if !status.success() {
        ko("Failed to edit SMTP configuration");
        return Err(Error::other("Failed to edit SMTP configuration"));
    }
    ok("SMTP configuration edited successfully.");
    Ok(())
}
pub fn create_smtp_config(
    from: &str,
    username: &str,
    password: &str,
    transport: &str,
    port: u16,
) -> Result<(), Error> {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    let path = format!("{home}/.config/lys");
    create_dir_all(path.as_str())?;
    let config_path = format!("{path}/smtp_config.yml");

    let config = SmtpConfig {
        from: from.to_string(),
        username: username.to_string(),
        password: password.to_string(),
        transport: transport.to_string(),
        port: port,
    };

    let file = std::fs::File::create(&config_path)?;
    serde_yaml::to_writer(file, &config).expect("Failed to write SMTP config");

    ok("SMTP configuration saved successfully.");
    Ok(())
}
