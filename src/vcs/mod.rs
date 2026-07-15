use crossterm::execute;
use crossterm::style::{Print, Stylize};
use crossterm::terminal::size;
use fluent_templates::{Loader, static_loader};
use std::io::stdout;
use sys_locale::get_locale;
use unic_langid::{LanguageIdentifier, langid};

use crate::vcs::commit::FileStatus;
pub mod branch;
pub mod commit;
pub mod conn;
pub mod db;
pub mod hooks;
pub mod init;
pub mod keys;
pub mod sync;
pub mod todo;
static_loader! {
    pub static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}
pub fn locale() -> LanguageIdentifier {
    let l = get_locale().unwrap_or(String::from("en-US"));
    l.parse().unwrap_or(langid!("en-US"))
}
/// Traduit une clé en string sans afficher
pub fn tt(lang: &LanguageIdentifier, key: &str) -> String {
    LOCALES.lookup(lang, key)
}

pub fn ok(t: &str) {
    let x = term_width();
    let symbol = "  ok  ";
    // 1. Calcul de la largeur réelle des caractères UTF-8
    let desc_width = t.chars().count();
    // 2. Définition des symboles et labels
    let icon = " *"; // Symbole UTF-8 (Checkmark)
    let padded_text = format!("{:^7}", symbol); // On aligne la chaîne pure d'abord
    let final_block = format!(
        "{} {} {}",
        "[".white().bold(),
        padded_text.green().bold(),
        "]".white().bold()
    );
    // 3. Calcul du padding sécurisé
    // On retire la largeur de l'icone (3), du label (2), des brackets (6) et des espaces
    let occupied_width = (desc_width + 17) as u16;
    let padding = x.saturating_sub(occupied_width);
    let _ = execute!(
        stdout(),
        // Icône en vert brillant
        Print(icon.green().bold()),
        Print(" "),
        Print(t),
        // Remplissage dynamique
        Print(" ".repeat(padding as usize)),
        Print(final_block),
        // Bloc de statut avec délimiteurs UTF-8
        Print("\n"),
    );
}

pub fn commit_created(hash: &str) {
    let x = term_width();

    let description = tt(&locale(), "commit-created");
    let padding =
        x.saturating_sub(hash.chars().count() as u16 + description.chars().count() as u16 + 7);

    let _ = execute!(
        stdout(),
        Print(" *".green().bold()),
        Print(description),
        Print(" ".repeat(padding as usize)),
        Print(" [ ".white().bold()),
        Print(hash.green().bold()),
        Print(" ]\n".trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn ok_commit_sent(hash: &str) {
    let x = term_width();

    let description = tt(&locale(), "commit-sent");
    let padding =
        x.saturating_sub(hash.chars().count() as u16 + description.chars().count() as u16 + 7);

    let _ = execute!(
        stdout(),
        Print(" *".green().bold()),
        Print(description),
        Print(" ".repeat(padding as usize)),
        Print(" [ ".white().bold()),
        Print(hash.green().bold()),
        Print(" ]\n".trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn ko_commit_sent(hash: &str) {
    let x = term_width();

    let description = tt(&locale(), "commit-no-sent");
    let padding =
        x.saturating_sub(hash.chars().count() as u16 + description.chars().count() as u16 + 7);

    let _ = execute!(
        stdout(),
        Print(" !".red().bold()),
        Print(description),
        Print(" ".repeat(padding as usize)),
        Print(" [ ".white().bold()),
        Print(hash.yellow().bold()),
        Print(" ]\n".trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn ko(t: &str) {
    let x = term_width();
    let symbol = "  ko  ";
    // 1. Calcul de la largeur réelle des caractères UTF-8
    let desc_width = t.chars().count();
    // 2. Définition des symboles et labels
    let icon = " !"; // Symbole UTF-8 (Checkmark)
    let padded_text = format!("{:^7}", symbol); // On aligne la chaîne pure d'abord
    let final_block = format!(
        "{} {} {}",
        "[".white().bold(),
        padded_text.red().bold(),
        "]".white().bold()
    );
    // 3. Calcul du padding sécurisé
    // On retire la largeur de l'icone (3), du label (2), des brackets (6) et des espaces
    let occupied_width = (desc_width + 17) as u16;
    let padding = x.saturating_sub(occupied_width);
    let _ = execute!(
        stdout(),
        Print(icon.red().bold()),
        Print(" "),
        Print(t),
        Print(" ".repeat(padding as usize)),
        Print(final_block),
        Print("\n"),
    );
}

fn term_width() -> u16 {
    size().map(|(w, _)| w).unwrap_or(80)
}

pub fn ok_audit(lang: &LanguageIdentifier, hash: &str) {
    let x = term_width();

    let y = tt(lang, "signature-is-valid");
    let padding = x.saturating_sub(hash.chars().count() as u16 + y.chars().count() as u16 + 7);
    let _ = execute!(
        stdout(),
        Print(" * ".green().bold()),
        Print(y),
        Print(" ".repeat(padding as usize)),
        Print(" [ ".white().bold()),
        Print(hash.green().bold()),
        Print(" ]\n".trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn ok_status(verb: &FileStatus) {
    let (p, symbol) = match verb {
        FileStatus::Modified(p, _) => (p.display().to_string(), String::from(" ~ ")),
        FileStatus::Deleted(p, _) => (p.display().to_string(), String::from(" - ")),
        FileStatus::New(p) => (p.display().to_string(), String::from(" + ")),
        FileStatus::Unchanged => (String::new(), String::from(" * ")),
    };

    let x = term_width();
    // 1. Calcul de la largeur réelle des caractères UTF-8
    let desc_width = p.chars().count();
    // 2. Définition des symboles et labels
    let icon = " * "; // Symbole UTF-8 (Checkmark)
    let padded_text = format!("{:^7}", symbol); // On aligne la chaîne pure d'abord
    let final_block = format!(
        "{} {} {}",
        "[".white().bold(),
        padded_text.green().bold(),
        "]".white().bold()
    );
    // 3. Calcul du padding sécurisé
    // On retire la largeur de l'icone (3), du label (2), des brackets (6) et des espaces
    let occupied_width = (desc_width + 14) as u16;
    let padding = x.saturating_sub(occupied_width);
    let _ = execute!(
        stdout(),
        // Icône en vert brillant
        Print(icon.green().bold()),
        Print(p),
        // Remplissage dynamique
        Print(" ".repeat(padding as usize)),
        Print(final_block),
        // Bloc de statut avec délimiteurs UTF-8
        Print("\n"),
    );
}
