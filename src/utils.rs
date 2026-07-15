use crate::vcs::{FileStatus, time_ago_cli};
use crossterm::execute;
use crossterm::style::{Print, Stylize};
use crossterm::{
    execute,
    style::{Print, Stylize},
    terminal::size,
};
use fluent_templates::{FluentValue, Loader};
use sqlite::Connection;
use sqlite::Error;
use sqlite::State;
use std::collections::HashMap;
use std::io::stdout;
use std::io::stdout;
use std::path::Path;
use std::process::Command;
use std::{fs::read_to_string, process::Stdio};
use unic_langid::LanguageIdentifier;

fluent_messages! {
    crate::LOCALES {
        locales: "./locales",
        fallback_language: "en-US",
    }
}
pub fn tt(
    lang: &LanguageIdentifier,
    key: &str,
    args: Option<HashMap<String, fluent_templates::FluentValue>>,
) -> String {
    LOCALES
        .lookup_with_args(lang, key, &args.unwrap_or_default())
        .unwrap_or_else(|| key.to_string())
}
pub fn ok(
    lang: &LanguageIdentifier,
    key: &str,
    args: Option<HashMap<String, fluent_templates::FluentValue>>,
) {
    let x = term_width();
    let desc = tt(lang, key, args);
    let desc_width = desc.chars().count();

    // 2. Définition des symboles et labels
    let icon = " * "; // Symbole UTF-8 (Checkmark)
    let status_label = "success";
    let brackets = (" [ ", " ] "); // Délimiteurs UTF-8 élégants

    // 3. Calcul du padding sécurisé
    // On retire la largeur de l'icone (3), du label (7), des brackets (6) et des espaces
    let occupied_width = (desc_width + 15) as u16;
    let padding = x.saturating_sub(occupied_width);
    let _ = execute!(
        stdout(),
        // Icône en vert brillant
        Print(icon.green().bold()),
        Print(desc),
        // Remplissage dynamique
        Print(" ".repeat(padding as usize)),
        // Bloc de statut avec délimiteurs UTF-8
        Print(brackets.0.white().bold()),
        Print(status_label.green().bold()),
        Print(brackets.1.trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn ko(lang: &LanguageIdentifier, key: &str, arg: Option<HashMap<String, FluentValue>>) {
    let x = term_width();
    let desc = tt(lang, key, args);
    let desc_width = desc.chars().count();

    // 2. Définition des symboles et labels
    let icon = " ! "; // Symbole UTF-8 (Checkmark)
    let status_label = "failure";
    let brackets = (" [ ", " ] "); // Délimiteurs UTF-8 élégants

    // 3. Calcul du padding sécurisé
    // On retire la largeur de l'icone (3), du label (7), des brackets (6) et des espaces
    let occupied_width = (desc_width + 15) as u16;
    let padding = x.saturating_sub(occupied_width);
    let _ = execute!(
        stdout(),
        // Icône en vert brillant
        Print(icon.green().bold()),
        Print(desc),
        // Remplissage dynamique
        Print(" ".repeat(padding as usize)),
        // Bloc de statut avec délimiteurs UTF-8
        Print(brackets.0.white().bold()),
        Print(status_label.red().bold()),
        Print(brackets.1.trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn get_branch_infos(
    conn: &Connection,
    branch_name: &str,
) -> Result<Option<(String, String, String)>, Error> {
    let query = "
    SELECT c.timestamp, c.author, c.message 
    FROM branches b 
    JOIN commits c ON b.head_commit_id = c.id 
    WHERE b.name = ?1
";
    let mut stmt = conn.prepare(query)?;
    stmt.bind((1, branch_name))?;

    if let Ok(State::Row) = stmt.next() {
        let timestamp: String = stmt.read(0)?;
        let author: String = stmt.read(1)?;
        let message: String = stmt.read(2)?;
        Ok(Some((timestamp, author, message)))
    } else {
        Ok(None)
    }
}
pub fn ok_merkle_hash(merkle_hash: &str) {
    let hash = &merkle_hash[0..7].green().to_string();
    let m = "  m  ".white().bold().to_string();
    let a = "[ ".white().bold().to_string();
    let b = " ]\n".white().bold().to_string();
    let _ = execute!(stdout(), Print(m), Print(a), Print(hash), Print(b));
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

pub fn ok_tag(tag: &str, description: &str, date: &str, hash: &str) {
    let x = term_width();

    let padding = x.saturating_sub(
        tag.chars().count() as u16
            + description.chars().count() as u16
            + time_ago_cli(date).chars().count() as u16
            + hash.chars().count() as u16
            + 15,
    );
    let _ = execute!(
        stdout(),
        Print(" * ".green().bold()),
        Print(format!("{tag} {description}").white().bold()),
        Print(" ( ".white().bold()),
        Print(time_ago_cli(date).green().bold()),
        Print(" ) ".white().bold()),
        Print(" ".repeat(padding as usize)),
        Print(" [ ".white().bold()),
        Print(hash.green().bold()),
        Print(" ]\n".white().bold()),
    );
}

pub fn ok_audit_commit(lang: &LanguageIdentifier, hash: &str) {
    let x = term_width();

    let mut args = HashMap::new();
    args.insert("hash".to_string(), &hash[0..7].into());
    let y = tt(lang, "signature-is-valid", args);
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

pub fn ko_verify(lang: &LanguageIdentifier, path: &str, hash: &str) {
    let x = term_width();
    let mut args: HashMap<String, FluentValue> = HashMap::new();
    args.insert("path".to_string(), path.into());
    args.insert("hash".to_string(), &hash[0..7].into());
    let description = tt(lang, "fingerprint-not-valid", args);
    let padding =
        x.saturating_sub(hash.chars().count() as u16 + description.chars().count() as u16 + 8);

    let _ = execute!(
        stdout(),
        Print(" ! ".red().bold()),
        Print(description),
        Print(" ".repeat(padding as usize)),
        Print(" [ ".white().bold()),
        Print(hash.yellow().bold()),
        Print(" ]\n".trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn ok_verify(path: &str, hash: &str) {
    let x = term_width();

    let description =
        format!("The fingerprint of the file '{path}' corresponds to the Merkle tree.");
    let padding =
        x.saturating_sub(hash.chars().count() as u16 + description.chars().count() as u16 + 8);

    let _ = execute!(
        stdout(),
        Print(" * ".green().bold()),
        Print(description),
        Print(" ".repeat(padding as usize)),
        Print(" [ ".white().bold()),
        Print(hash.green().bold()),
        Print(" ]\n".trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn missing_verify(path: &str, hash: &str) {
    let x = term_width();

    let description =
        format!("The fingerprint of the file '{path}' is missing in the Merkle tree.");
    let padding =
        x.saturating_sub(hash.chars().count() as u16 + description.chars().count() as u16 + 8);

    let _ = execute!(
        stdout(),
        Print(" ! ".red().bold()),
        Print(description),
        Print(" ".repeat(padding as usize)),
        Print(" [ ".white().bold()),
        Print(hash.yellow().bold()),
        Print(" ]\n".trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn commit_created(hash: &str) {
    let x = term_width();

    let description = " Committed successfully ";
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

pub fn tag_created(hash: &str) {
    let x = term_width();

    let description = " tagged successfully ";
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

pub fn ko_audit_commit(hash: &str) {
    let x = term_width();
    let description = " Signature is not valid ";
    let padding =
        x.saturating_sub(hash.chars().count() as u16 + description.chars().count() as u16 + 6);

    let _ = execute!(
        stdout(),
        Print(" !".red().bold()),
        Print(description),
        Print(" ".repeat(padding as usize)),
        Print(" [ ".white().bold()),
        Print(hash.red().bold()),
        Print(" ]\n".trim_end().white().bold()),
        Print("\n"),
    );
}

pub fn run_hooks() -> Result<(), Box<dyn std::error::Error>> {
    let lys_file = Path::new("lys");
    if !lys_file.exists() {
        return Ok(());
    }

    let content = read_to_string(lys_file)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        ok(&format!("Running hook: {line}"));

        let status = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", line])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?
        } else {
            Command::new("sh")
                .args(["-c", line])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?
        };

        if !status.success() {
            return Err(format!("Hook failed: {line}").into());
        }
    }
    ok("code can be commited.");
    Ok(())
}

fn term_width() -> u16 {
    size().map(|(w, _)| w).unwrap_or(80)
}
