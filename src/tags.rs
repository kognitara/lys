use crate::branch::get_branch_head_info;
use crate::branch::get_current_branch;
use crate::db::Season;
use crate::utils::ko;
use crate::utils::ok;
use crate::utils::ok_tag;
use crate::utils::tag_created;
use crate::vcs::status;
use blake3::Hasher;
use chrono::Datelike;
use chrono::Local;
use flate2::Compression;
use flate2::write::GzEncoder;
use glob::glob;
use sqlite::Connection;
use sqlite::State;
use std::fs;
use std::fs::File;
use std::fs::create_dir_all;
use std::io::Error;
use std::path::Path;
use tar::{Builder, Header};
use zstd::Decoder;

pub fn audit_tags() -> Result<(), Error> {
    // Adapte l'erreur selon ton architecture
    let path = Path::new(".lys/tags");
    create_dir_all(path.to_path_buf())?;

    let dir = path
        .join(Local::now().year().to_string())
        .join(Season::current().to_string());
    create_dir_all(dir.to_path_buf())?;
    for archive in glob(dir.join("*.b3").display().to_string().as_str()).expect("") {
        let x = archive
            .expect("archive not found")
            .canonicalize()
            .expect("a");

        let f = fs::read_to_string(x.to_path_buf())?;
        let part = f.split_whitespace().collect::<Vec<&str>>();
        let filename = dir.join(part.last().expect("failed to find file"));
        let hash = part.first().expect("").trim();
        let file = fs::read(filename.to_path_buf()).expect("failed to read");
        let mut h = Hasher::new();
        h.update(&file);
        let archive_hash = h.finalize().to_hex();
        if archive_hash.eq(hash) {
            ok(format!(
                "{} is valid",
                Path::new(&filename)
                    .file_name()
                    .expect("failed to get filename")
                    .display()
            )
            .as_str());
        } else {
            ko(format!(
                "{} is invalid",
                Path::new(&filename)
                    .file_name()
                    .expect("failed to get filename")
                    .display()
            )
            .as_str());
        }
    }
    Ok(())
}
/// Crée une archive tar.gz d'un commit spécifique en lisant directement la BDD
pub fn create_archive(conn: &Connection, commit_id: i64, archive_path: &str) -> Result<(), Error> {
    // Adapte l'erreur selon ton architecture
    let path = Path::new(".lys/tags");
    create_dir_all(path.to_path_buf())?;
    let dir = path
        .join(Local::now().year().to_string())
        .join(Season::current().to_string());
    create_dir_all(dir.to_path_buf())?;
    let arch = dir.join(archive_path);
    // 1. Préparer la création du fichier final .tar.gz
    let tar_gz_file = File::create(arch.to_path_buf())?;
    let enc = GzEncoder::new(tar_gz_file, Compression::default());
    let mut tar_builder = Builder::new(enc);

    // 2. Préparer la requête récursive pour parcourir l'arbre complet du commit
    let query = "
        WITH RECURSIVE tree_walk(path, hash) AS (
            -- Étape 1 : Base de la récursion (les fichiers/dossiers à la racine du commit)
            SELECT name AS path, hash 
            FROM tree_nodes 
            WHERE parent_tree_hash = (SELECT tree_hash FROM commits WHERE id = ?)
            
            UNION ALL
            
            -- Étape 2 : Récursion (on descend dans les sous-dossiers en construisant le chemin)
            SELECT tw.path || '/' || tn.name, tn.hash
            FROM tree_nodes tn
            JOIN tree_walk tw ON tn.parent_tree_hash = tw.hash
        )
        -- Étape 3 : On filtre uniquement les fichiers (blobs) et on récupère leur contenu
        SELECT w.path, b.size, b.content 
        FROM tree_walk w
        JOIN store.blobs b ON w.hash = b.hash
    ";

    let mut stmt = conn.prepare(query).expect("failed to prepare");

    // 3. Binder l'ID du commit (l'index commence à 1 dans sqlite)
    stmt.bind((1, commit_id)).expect("failed to bind");

    // 4. Parcourir chaque fichier du manifest avec l'API sqlite
    while let Ok(State::Row) = stmt.next() {
        // Lecture des colonnes avec le bon type
        let file_path: String = stmt.read(0).expect("failed to read");
        let original_size: i64 = stmt.read(1).expect("failed to read"); // On lit en i64 d'abord
        let zlib_content: Vec<u8> = stmt.read(2).expect("failed to read");
        let mut decoder = Decoder::new(&zlib_content[..]).expect("failed to read");
        let mut header = Header::new_gnu();
        header.set_size(original_size as u64); // Conversion en u64 requise par tar
        header.set_mode(0o644); // Permissions de base (rw-r--r--)
        header.set_cksum(); // Calcul du checksum obligatoire

        // 7. Ajouter le fichier et son contenu décompressé dans l'archive
        tar_builder.append_data(&mut header, Path::new(&file_path), &mut decoder)?;
    }

    // 8. Finaliser l'écriture de l'archive
    tar_builder.into_inner()?.finish()?;

    let file = fs::read(arch.to_path_buf())?;
    let mut h = Hasher::new();
    h.update(&file);
    let archive_hash = h.finalize().to_hex();

    // 3. Signer le hash avec ta fonction existante
    match crate::crypto::sign_message(Path::new("."), &archive_hash) {
        Ok(signature_hex) => {
            // 4. Sauvegarder la signature dans un fichier détaché (.sig)
            let sig_path = format!("{}.sig", arch.display());
            fs::write(sig_path.as_str(), signature_hex.as_str())?;
            fs::write(
                format!("{}.b3", arch.display()).as_str(),
                format!(
                    "{archive_hash}  {}",
                    arch.file_name().expect("failed to get filename").display()
                ),
            )?;
            ok("Archive signed");
        }
        Err(e) => {
            return Err(Error::other(format!("Sign failed : {e}")));
        }
    }
    Ok(())
}

pub fn tag_create(conn: &Connection, version: &str, message: Option<&str>) -> Result<(), Error> {
    let current_branch = get_current_branch(conn).expect("failed to get current branch");
    let pending_changes = status(conn, ".", &current_branch).expect("failed to get status");

    // Si le vecteur/tableau retourné n'est pas vide, on stoppe tout
    if !pending_changes.is_empty() {
        return Err(Error::other(
            "Cannot create tag: you have uncommitted changes. Please commit them first.",
        ));
    }
    // 1. On récupère le commit actuel (HEAD)
    let (head_id, head_hash) =
        get_branch_head_info(conn, &current_branch).map_err(|e| Error::other(e.to_string()))?;

    if head_id.is_none() {
        return Err(Error::other(
            "Cannot tag an empty branch. Commit something first.",
        ));
    }

    // 2. On insère le tag
    let query = "INSERT INTO tags (id, version, message) VALUES (?, ?, ?)";
    let mut stmt = conn
        .prepare(query)
        .map_err(|e| Error::other(e.to_string()))?;
    stmt.bind((1, head_id.unwrap())).unwrap();
    stmt.bind((2, version)).unwrap();
    stmt.bind((3, message)).unwrap();

    match stmt.next() {
        Ok(_) => {
            create_archive(
                conn,
                head_id.expect("failed to get commit id"),
                format!("{version}.tar.gz").as_str(),
            )
            .expect("failed to create archive");
            ok(format!("{version}.tar.gz created").as_str());
            tag_created(&head_hash[0..7]);
        }
        Err(_) => return Err(Error::other(format!("Tag '{version}' already exists."))),
    }
    Ok(())
}

pub fn tag_list(conn: &Connection) -> Result<(), Error> {
    // On joint avec la table commits pour afficher le hash correspondant
    let query = "
        SELECT t.version, t.message, t.created_at, c.hash
        FROM tags t
        JOIN commits c ON t.id = c.id
        ORDER BY t.created_at ASC
    ";
    let mut stmt = conn
        .prepare(query)
        .map_err(|e| Error::other(e.to_string()))?;

    let mut count = 0;
    while let Ok(State::Row) = stmt.next() {
        let name: String = stmt.read("version").unwrap();
        let desc: Option<String> = stmt.read("message").unwrap_or(None);
        let hash: String = stmt.read("hash").unwrap();
        let date: String = stmt.read("created_at").unwrap();
        let desc_str = desc.unwrap_or_else(|| String::from("no description"));
        ok_tag(name.as_str(), desc_str.as_str(), date.as_str(), &hash[..7]);
        count += 1;
    }
    if count == 0 {
        ok("no tags yet");
    }
    Ok(())
}
