use crate::utils::{ko, ok};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

pub fn send(to: &str, subject: &str, message: &str) {
    let from = std::env::var("SMTP_FROM").expect("SMTP_FROM missing");
    let smtp_username = std::env::var("SMTP_USERNAME").expect("SMTP_USERNAME missing");
    let smtp_password = std::env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD missing");
    let smtp_transport = std::env::var("SMTP_TRANSPORT").expect("SMTP_TRANSPORT missing");
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
    let mailer = SmtpTransport::relay(&smtp_transport)
        .unwrap()
        .credentials(creds)
        .build();

    // 4. Envoi et gestion du résultat
    match mailer.send(&email) {
        Ok(_) => ok("Email send successfully"),
        Err(_) => ko("Failed to send email"),
    }
}
