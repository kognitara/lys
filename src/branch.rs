use crate::utils::{ko, ok};
use crate::vcs::get_blob_bytes_by_hash;
use crate::vcs::get_manifest_map;
use crate::vcs::status;
use anyhow::Error;
use anyhow::anyhow;
use sqlite::{Connection, State};
use std::fs::create_dir_all;
use std::path::Path;

pub fn list_branches(conn: &Connection) -> Vec<String> {
    get_branches(conn)
}

pub fn delete_branch(conn: &Connection, branch: &str) -> Result<(), Error> {
    if get_current_branch(conn)? != branch && list_branches(conn).contains(&branch.to_string()) {
        let mut stmt = conn.prepare("DELETE FROM branches WHERE name = ?")?;
        stmt.bind((1, branch))?;
        stmt.next()?;
        ok(format!("{branch} deleted successfully").as_str());
    }
    Ok(())
}
pub fn get_branches(conn: &Connection) -> Vec<String> {
    let mut out = Vec::new();
    let mut stmt = match conn.prepare("SELECT name FROM branches ORDER BY name") {
        Ok(s) => s,
        Err(_) => return out,
    };
    while let Ok(State::Row) = stmt.next() {
        if let Ok(name) = stmt.read::<String, _>(0) {
            out.push(name);
        }
    }
    out
}

pub fn get_current_branch(conn: &Connection) -> Result<String, Error> {
    let query = "SELECT value FROM config WHERE key = 'current_branch'";
    let mut statement = conn.prepare(query)?;
    if let Ok(State::Row) = statement.next() {
        let branch_name: String = statement.read("value")?;
        Ok(branch_name)
    } else {
        // Fallback si la config est cassée, mais ça ne devrait pas arriver
        Ok(String::from("main"))
    }
}
pub fn branch_head_id(conn: &Connection, branch: &str) -> Result<Option<i64>, Error> {
    let query =
        "SELECT c.id FROM branches b JOIN commits c ON b.head_commit_id = c.id WHERE b.name = ?";
    let mut stmt = conn.prepare(query)?;
    stmt.bind((1, branch))?;
    if let Ok(State::Row) = stmt.next() {
        Ok(Some(stmt.read::<i64, _>(0)?))
    } else {
        Ok(None)
    }
}

pub fn hotfix_start(conn: &Connection, name: &str) -> Result<(), Error> {
    let branch_name = format!("hotfix/{name}");
    if get_current_branch(conn)?.ne(&branch_name) && !list_branches(conn).contains(&branch_name) {
        let source_branch = "main"; // CONTRAINTE : Un hotfix part toujours de la prod

        // 1. On vérifie qu'on part bien de 'main' pour avoir la base saine
        let (main_id, _) = get_branch_head_info(conn, source_branch)?;
        if main_id.is_none() {
            return Err(anyhow!("No main branch has been founded").into());
        }
        // 2. On crée la branche manuellement (sans utiliser create_branch qui utilise HEAD)
        let query = "INSERT INTO branches (name, head_commit_id) VALUES (?, ?)";
        let mut stmt = conn.prepare(query)?;
        stmt.bind((1, branch_name.as_str()))?;
        stmt.bind((2, main_id.unwrap()))?;

        match stmt.next() {
            Ok(_) => {
                checkout(conn, &branch_name)?;
                ok(&format!(
                    "Hotfix started: Switched to '{branch_name}' from 'main'"
                ));
                Ok(())
            } // Création OK
            Err(_) => Err(anyhow!("hotfix creation failed")),
        }
    } else {
        Err(anyhow!("hotfix already exists"))
    }
}

pub fn hotfix_finish(conn: &Connection, name: &str) -> Result<(), Error> {
    // C'est la même logique que feature_finish, mais sémantiquement distinct
    let hotfix_branch = format!("hotfix/{name}");
    let target_branch = "main";

    if list_branches(conn).contains(&hotfix_branch) {
        let (hf_head_id, _) = get_branch_head_info(conn, &hotfix_branch)?;
        if hf_head_id.is_none() {
            return Err(anyhow!("hotfix not exist"));
        }
        ok(format!("Switching to '{target_branch}' to apply hotfix...").as_str());
        checkout(conn, target_branch)?;

        // Fast-Forward Merge
        let query = "UPDATE branches SET head_commit_id = ? WHERE name = ?";
        let mut stmt = conn.prepare(query)?;
        stmt.bind((1, hf_head_id.unwrap()))?;
        stmt.bind((2, target_branch))?;
        stmt.next()?;

        ok("Hotfix applied to main");
        delete_branch(conn, hotfix_branch.as_str())?;
        ok(&format!("Hotfix '{name}' finished and branch deleted."));
        ok(&format!("You are now on : '{target_branch}'"));
        Ok(())
    } else {
        Err(anyhow!("Failed to finish hotfix"))
    }
}

pub fn feature_start(conn: &Connection, name: &str) -> Result<(), Error> {
    let branch_name = format!("feature/{name}");
    if get_current_branch(conn)?.ne(branch_name.as_str())
        && !list_branches(conn).contains(&branch_name)
    {
        create_branch(conn, &branch_name)?;
        checkout(conn, &branch_name)?;
        ok(&format!("Flow started: You are now on '{branch_name}'"));
        Ok(())
    } else {
        Err(anyhow!("Failed to start feature"))
    }
}

pub fn feature_finish(conn: &Connection, name: &str) -> Result<(), Error> {
    let feat_branch = format!("feature/{name}");
    let target_branch = "main";
    if get_current_branch(conn)?.ne(feat_branch.as_str())
        && !list_branches(conn).contains(&feat_branch)
    {
        // 1. Sécurité : On vérifie que la branche feature existe
        let (feat_head_id, _) = get_branch_head_info(conn, &feat_branch)?;
        if feat_head_id.is_none() {
            return Err(anyhow::anyhow!("main branch not exist"));
        }
        // 2. On bascule sur 'main' pour préparer la fusion
        ok(format!("Switching to '{target_branch}' to merge changes...").as_str());
        checkout(conn, target_branch)?;

        // 3. LE FAST-FORWARD (L'optimisation ultime)
        // Au lieu de calculer un diff, on déplace juste le pointeur de main sur la tête de la feature
        let query = "UPDATE branches SET head_commit_id = ? WHERE name = ?";
        let mut stmt = conn.prepare(query)?;
        stmt.bind((1, feat_head_id.unwrap()))?;
        stmt.bind((2, target_branch))?;
        stmt.next()?;
        ok("Fast-forward merge complete");
        delete_branch(conn, &feat_branch.as_str())?;
        ok(&format!("Feat '{name}' finished and branch deleted."));
        Ok(())
    } else {
        Err(anyhow!("failed to finnish feature"))
    }
}

pub fn checkout(conn: &Connection, target_ref: &str) -> Result<(), Error> {
    // 1. VÉRIFICATION DE SÉCURITÉ
    let current_dir = std::env::current_dir()?;
    let current_branch = get_current_branch(conn).unwrap_or("DETACHED".to_string());

    // Si on est déjà dessus (et que ce n'est pas un checkout forcé sur un hash), on skip
    if current_branch == target_ref {
        ok(&format!("Already on '{target_ref}'"));
        return Ok(());
    }

    let status_list = status(conn, current_dir.to_str().unwrap(), &current_branch)?;
    if !status_list.is_empty() {
        ok("Your changes would be overwritten by checkout.");
        ok("Please commit your changes or stash them first.");
        return Ok(());
    }

    // 2. PRÉPARATION DES DONNÉES (C'est ici qu'on change la logique !)
    let (current_head_id, _) = get_branch_head_info(conn, &current_branch)?;

    // A. Est-ce une BRANCHE ?
    let (branch_head_id, _) = get_branch_head_info(conn, target_ref)?;

    // B. Sinon, est-ce un HASH (Time Travel) ?
    let target_head_id = if branch_head_id.is_some() {
        branch_head_id
    } else {
        get_commit_id_by_hash(conn, target_ref)?
    };

    // Si introuvable ni en branche, ni en commit
    if target_head_id.is_none() {
        return Err(anyhow!(
            "Reference '{target_ref}' (branch or commit) not found."
        ));
    }
    // On charge les deux manifestes en mémoire pour comparer
    let current_files = get_manifest_map(conn, current_head_id)?;
    let target_files = get_manifest_map(conn, target_head_id)?;
    ok(format!("Switched to branch '{target_ref}'").as_str());

    // 3. MISE À JOUR DU DISQUE (Différentiel)

    // A. Gérer les AJOUTS et MODIFICATIONS (Target vs Current)
    for (path, (target_hash, _)) in &target_files {
        let should_write = match current_files.get(path) {
            Some((current_hash, _)) => current_hash != target_hash, // Modifié
            None => true,                                           // Nouveau fichier
        };

        if should_write {
            // On récupère le contenu binaire depuis le store
            if let Some(content) = get_blob_bytes_by_hash(conn, target_hash)?
                && let Some(parent) = Path::new(path).parent()
            {
                create_dir_all(parent).expect("failed to create directory");
                std::fs::write(path, content).expect("failed to write content");
            }
        }
    }
    // B. Gérer les SUPPRESSIONS (Ce qui est dans Current mais plus dans Target)
    for path in current_files.keys() {
        if !target_files.contains_key(path) && Path::new(path).exists() {
            std::fs::remove_file(path).expect("failed to remove the file");
            // Optionnel : Supprimer les dossiers vides parents
        }
    }
    let query = "INSERT INTO config (key, value) VALUES ('current_branch', ?) 
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value";
    let mut stmt = conn.prepare(query)?;

    if branch_head_id.is_some() {
        // C'est une vraie branche
        stmt.bind((1, target_ref))?;
    } else {
        ok(format!("You are in 'Detached HEAD' state (viewing commit {target_ref}).").as_str());
        stmt.bind((1, "DETACHED"))?;
    }
    stmt.next()?;
    Ok(())
}
pub fn get_branch_head_info(
    conn: &Connection,
    branch: &str,
) -> Result<(Option<i64>, String), Error> {
    // 1. Base actuelle
    let query = "SELECT c.id, c.hash FROM branches b JOIN commits c ON b.head_commit_id = c.id WHERE b.name = ?";
    let mut stmt = conn.prepare(query)?;
    stmt.bind((1, branch))?;

    if let Ok(State::Row) = stmt.next() {
        return Ok((Some(stmt.read("id")?), stmt.read("hash")?));
    }

    // 2. Repli sur la base 'old' (la "Dernière Base Connue")
    let query_old = "SELECT c.hash FROM old.branches b JOIN old.commits c ON b.head_commit_id = c.id WHERE b.name = ?";
    if let Ok(mut stmt_old) = conn.prepare(query_old) {
        stmt_old.bind((1, branch))?;
        if let Ok(State::Row) = stmt_old.next() {
            // On renvoie l'ID à None (car l'ID de 'old' n'existe pas ici) mais le HASH pour le chaînage
            return Ok((None, stmt_old.read("hash")?));
        }
    }
    Ok((None, String::new()))
}

pub fn get_commit_id_by_hash(conn: &Connection, partial_hash: &str) -> Result<Option<i64>, Error> {
    // On cherche un hash qui COMMENCE par la chaîne donnée (LIKE 'abc%')
    let query = "SELECT id FROM commits WHERE hash LIKE ? || '%' LIMIT 1";
    let mut stmt = conn.prepare(query)?;
    stmt.bind((1, partial_hash))?;

    if let Ok(State::Row) = stmt.next() {
        Ok(stmt.read("id")?)
    } else {
        Ok(None)
    }
}
pub fn create_branch(conn: &Connection, new_branch_name: &str) -> Result<(), Error> {
    // 1. On récupère la branche actuelle et son commit ID
    let current_branch = get_current_branch(conn).expect("failed to get current branch");
    let (head_id, _) = get_branch_head_info(conn, &current_branch)?;

    if let Some(id) = head_id {
        // 2. On insère la nouvelle étiquette pointant vers le MEME commit
        let query = "INSERT INTO branches (name, head_commit_id) VALUES (?, ?)";
        let mut stmt = conn.prepare(query)?;
        stmt.bind((1, new_branch_name))?;
        stmt.bind((2, id))?;

        match stmt.next() {
            Ok(_) => ok(&format!("Branch '{new_branch_name}' created.")),
            Err(_) => ko(format!("Error: branch '{new_branch_name}' already exists.").as_str()),
        }
    } else {
        ok("Cannot branch from an empty repository. Commit something first.");
    }
    Ok(())
}
