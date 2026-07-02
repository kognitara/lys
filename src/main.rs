use crate::Language::{C, CSharp, Cpp, D, Haskell, Js, Php, Python, Rust, Typescript};
use crate::branch::{
    checkout, create_branch, delete_branch, feature_finish, feature_start, get_current_branch,
    hotfix_finish, hotfix_start, list_branches,
};
use crate::chat::list_messages;
use crate::chat::send_message;
use crate::commit::author;
use crate::crypto::generate_keypair;
use crate::db::connect_lys;
use crate::db::{LYS_INIT, set_config};
use crate::import::extract_repo_name;
use crate::lysrc::Lysrc;
use crate::shell::Shell;
use crate::tags::{audit_tags, tag_create, tag_list};
use crate::utils::ko;
use crate::utils::ok_merkle_hash;
use crate::utils::run_hooks;
use crate::utils::{get_branch_infos, ok};
use crate::vcs::{internal_pager, time_ago_cli};
use clap::value_parser;
use clap::{Arg, ArgAction, Command};
use crossterm::cursor::MoveTo;
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use dotenv::dotenv;
use inquire::{Confirm, Editor, Select, Text};
use inquire::{DateSelect, Password};
use sqlite::State;
use std::env::current_dir;
use std::fmt::Display;
use std::fs::File;
use std::fs::read_to_string;
use std::io::{Error, Write, stdout};
use std::path::MAIN_SEPARATOR_STR;
use std::path::Path;
use std::process::{Command as Cmd, Stdio, exit};
use tabled::Table;
use tabled::builder::Builder;
use tabled::settings::Style;

const PROJ_GEN_SUCCESS: &str = "The project has been generated successfully";
pub mod branch;
pub mod chat;
pub mod commit;
pub mod crypto;
pub mod db;
pub mod email;
pub mod import;
pub mod lysrc;
pub mod mount;
pub mod qr;
pub mod shell;
pub mod tags;
pub mod todo;
pub mod tree;
pub mod utils;
pub mod vcs;
pub mod web;

fn cli() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(Command::new("init").about("Initialize the current directory"))
        .subcommand(Command::new("reinit").about("Re-initialize the current directory"))
        .subcommand(Command::new("new").about("Create a new lys project"))
        .subcommand(Command::new("email").about("Write and send an email").subcommands([
            Command::new("write").about("Write a new email"),
            Command::new("configure").about("Create the SMTP settings config file"),
            Command::new("edit").about("Edit the SMTP settings"),
        ]))
        .subcommand(
            Command::new("web")
                .about("Run or get web info")
                .subcommand(Command::new("edit").about("edit lysrc with $EDITOR"))
                .subcommand(Command::new("info").about("display lysrc informations"))
                .subcommand(Command::new("run").about("Run web server").arg(
                    Arg::new("port")
                        .short('p')
                        .default_value("3000")
                        .action(ArgAction::Set),
                )
                .arg(
                    Arg::new("spotify")
                        .short('s')
                        .long("spotify")
                        .help("Music URL (Spotify or YouTube Music) to display on the home page")
                        .action(ArgAction::Set),
                )
                .arg(
                    Arg::new("video")
                        .short('v')
                        .long("video")
                        .help("YouTube Video URL to display as banner on the home page")
                        .action(ArgAction::Set),
                )
                .arg(
                    Arg::new("banner")
                        .short('b')
                        .long("banner")
                        .help("Image URL to display as banner on the home page")
                        .action(ArgAction::Set),
                )
                .arg(
                    Arg::new("title")
                        .long("title")
                        .help("Custom site title to display in header and browser tab")
                        .action(ArgAction::Set),
                )
                .arg(
                    Arg::new("subtitle")
                        .long("subtitle")
                        .help("Custom subtitle/description to display under the header title")
                        .action(ArgAction::Set),
                )
                .arg(
                    Arg::new("footer")
                        .long("footer")
                        .help("Custom footer HTML to display at the bottom of pages")
                        .action(ArgAction::Set),
                )
                .arg(
                    Arg::new("homepage")
                        .long("homepage")
                        .help("URL to the project's homepage")
                        .action(ArgAction::Set),
                )
                .arg(
                    Arg::new("documentation")
                        .long("documentation")
                        .help("URL to the project's documentation")
                        .action(ArgAction::Set),
                ),
        ))
        .subcommand(
            Command::new("branch")
                .about("Manage branches")
                .subcommand(Command::new("list").about("List existing branches"))
                .subcommand(
                    Command::new("checkout")
                        .about("Checkout on existing branche")
                        .arg(Arg::new("branch").required(true).action(ArgAction::Set)),
                )
                .subcommand(
                    Command::new("delete")
                        .about("Delete a branch")
                        .arg(Arg::new("branch").required(true).action(ArgAction::Set)),
                )
                .subcommand(
                    Command::new("create")
                        .about("Create a new branch")
                        .arg(
                            Arg::new("feat")
                                .long("feat")
                                .action(ArgAction::SetTrue)
                                .required(false),
                        )
                        .arg(
                            Arg::new("hotfix")
                                .long("hotfix")
                                .action(ArgAction::SetTrue)
                                .required(false),
                        )
                        .arg(
                            Arg::new("name")
                                .required(true)
                                .action(ArgAction::Set)
                                .help("Name of branch to create"),
                        ),
                ),
        )
        .subcommand(
            Command::new("verify")
                .about("Check repository integrity and missing blobs")
                .arg(
                    Arg::new("deep")
                        .long("deep")
                        .action(ArgAction::SetTrue)
                        .help("Recalculate Blake3 checksums for every blob (Slower but safer)"),
                ),
        )
        .subcommand(Command::new("summary").about("Show working directory infos"))
        .subcommand(Command::new("status").about("Show changes in working directory"))
        .subcommand(Command::new("push").about("Push local commits to a remote architect"))
        .subcommand(Command::new("pull").about("Pull commits from a remote architect"))
        .subcommand(
            Command::new("prune").about(
                "Maintain repository health by removing old history and reclaiming disk space.",
            ),
        )
        .subcommand(
            Command::new("shell")
                .about("Open a temporary shell with the code mounted")
                .arg(
                    Arg::new("ref")
                        .help("Reference to mount (default: HEAD)")
                        .required(false)
                        .action(ArgAction::Set),
                ),
        )
        .subcommand(
            Command::new("mount")
                .about("Mount a specific version or the current head to a directory")
                .arg(
                    Arg::new("target")
                        .help("The mount point (e.g., /mnt/lys_project)")
                        .required(true)
                        .action(ArgAction::Set),
                )
                .arg(
                    Arg::new("ref")
                        .short('r')
                        .long("ref")
                        .help("Branch, tag or commit hash to mount (default: current HEAD)")
                        .action(ArgAction::Set),
                ),
        )
        .subcommand(
            Command::new("tree").about("Show repository").arg(
                Arg::new("color")
                    .help("colorize tree or not")
                    .required(false)
                    .default_value("false")
                    .value_parser(value_parser!(String)),
            ),
        )
        .subcommand(
            Command::new("import")
                .about("Import a Git repository into Lys")
                .arg(Arg::new("url").required(true).help("Git repository URL"))
                .arg(
                    Arg::new("depth")
                        .long("depth")
                        .value_parser(value_parser!(i32))
                        .help("Number of commits to import"),
                )
                .arg(
                    Arg::new("recent")
                        .long("recent")
                        .action(ArgAction::SetTrue)
                        .help("Only import the last 2 years of history (Lean mode)"),
                ),
        )
        .subcommand(
            Command::new("keygen").about("Generate Ed25519 identity keys for signing commits"),
        )
        .subcommand(
            Command::new("serve")
                .about("Start the Silex Node (Daemon) to receive atoms")
                .arg(Arg::new("port").short('p').default_value("3000")),
        )
        .subcommand(Command::new("audit").about("Verify integrity of commit signatures"))
        .subcommand(
            Command::new("log")
                .about("Show commit logs")
                .arg(
                    Arg::new("page")
                        .short('p')
                        .long("page")
                        .value_parser(value_parser!(usize))
                        .default_value("1")
                        .help("Page number (default: 1)"),
                )
                .arg(
                    Arg::new("limit")
                        .short('n')
                        .long("limit")
                        .value_parser(value_parser!(usize))
                        .default_value("120") // Ta demande spécifique
                        .help("Number of commits per page"),
                ),
        )
        .subcommand(Command::new("diff").about("Show changes between working tree and last commit"))
        .subcommand(
            Command::new("clone")
                .about("Clone a Git repository into a new lys repository")
                .arg(
                    Arg::new("url")
                        .required(true)
                        .help("The git URL (https://...)")
                        .action(ArgAction::Set),
                )
                // Optionnel : permettre de forcer un nom de dossier différent
                .arg(
                    Arg::new("name")
                        .required(false)
                        .help("Target directory name"),
                )
                .arg(
                    Arg::new("depth")
                        .long("depth")
                        .short('d')
                        .value_parser(value_parser!(i32))
                        .help("Truncate history to the specified number of commits"),
                ),
        )
        .subcommand(Command::new("health").about("Check the source code"))
        .subcommand(
            Command::new("todo")
                .about("Manage project tasks")
                                .subcommand(
                                    Command::new("add")
                                        .about("Add todos")
                                        .arg(Arg::new("title").help("Todo title").required(false).action(ArgAction::Set))
                                        .arg(Arg::new("user").short('u').long("user").help("Assign to").required(false).action(ArgAction::Set))
                                        .arg(Arg::new("due").short('d').long("due").help("Due date (YYYY-MM-DD)").required(false).action(ArgAction::Set))
                                        .arg(Arg::new("description").short('m').long("message").help("Description").required(false).action(ArgAction::Set))
                                )
                .subcommand(
                    Command::new("start").about("Start a todo").arg(
                        Arg::new("id")
                            .required(true)
                            .value_parser(value_parser!(i64)),
                    ),
                )
                .subcommand(Command::new("list").about("List all todos"))
                .subcommand(
                    Command::new("close").about("Close a todo").arg(
                        Arg::new("id")
                            .required(true)
                            .value_parser(value_parser!(i64)),
                    ),
                ),
        )
        .subcommand(Command::new("commit").about("Record changes to the repository"))
        .subcommand(
            Command::new("restore")
                .about("Discard changes in working directory")
                .arg(
                    Arg::new("path")
                        .help("The file to restore")
                        .required(true)
                        .action(ArgAction::Set),
                ),
        )
        .subcommand(
            Command::new("chat")
                .about("Chat with the team")
                .subcommand(Command::new("write").about("write and send a message"))
                .subcommand(Command::new("list").about("list messages")),
        )
        .subcommand(
            Command::new("backup")
                .about("Backup repository to a destination (USB, Drive...)")
                .arg(
                    Arg::new("path")
                        .required(true)
                        .action(ArgAction::Set)
                        .help("Destination path"),
                ),
        )
        .subcommand(
            Command::new("switch")
                .about("Switch branches")
                .arg(Arg::new("branch").required(true).action(ArgAction::Set)),
        )
        .subcommand(
            Command::new("feat")
                .about("Manage feature branches")
                .subcommand(
                    Command::new("start")
                        .about("Start a new feature")
                        .arg(Arg::new("name").required(true).action(ArgAction::Set)),
                )
                .subcommand(
                    Command::new("finish")
                        .about("Merge and close a feature")
                        .arg(Arg::new("name").required(true).action(ArgAction::Set)),
                ),
        )
        .subcommand(
            Command::new("hotfix")
                .about("Manage hotfix branches")
                .subcommand(
                    Command::new("start")
                        .about("Start a critical fix from main")
                        .arg(Arg::new("name").required(true).action(ArgAction::Set)),
                )
                .subcommand(
                    Command::new("finish")
                        .about("Apply fix to main and close")
                        .arg(Arg::new("name").required(true).action(ArgAction::Set)),
                ),
        )
        .subcommand(
            Command::new("tag")
                .about("Manage version tags")
                .subcommand(
                    Command::new("create")
                        .about("Create a new tag at HEAD")
                        .arg(Arg::new("name").required(true).action(ArgAction::Set))
                        .arg(
                            Arg::new("message")
                                .short('m')
                                .help("Description")
                                .action(ArgAction::Set),
                        ),
                )
                .subcommand(Command::new("list").about("List all tags"))
                .subcommand(Command::new("audit").about("Verify all tags")),
        )
        .subcommand(
            Command::new("spotify")
                .about("Set the Music album/track to display on the home page")
                .arg(
                    Arg::new("url")
                        .required(true)
                        .help("Music URL (Spotify or YouTube Music)"),
                ),
        )
        .subcommand(
            Command::new("video")
                .about("Set the YouTube video banner to display on the home page")
                .arg(Arg::new("url").required(true).help("YouTube Video URL")),
        )
        .subcommand(
            Command::new("banner")
                .about("Set the image banner to display on the home page")
                .arg(Arg::new("url").required(true).help("Image URL")),
        )
}

fn perform_commit() -> Result<(), Error> {
    let current_dir = current_dir()?;
    let current_dir_str = current_dir.to_str().unwrap();

    if !Path::new(".lys").exists() {
        eprintln!(
            "Error: Not a lys repository. Please run 'lys init' to initialize the repository."
        );
        exit(1);
    }

    let connection =
        connect_lys(Path::new(current_dir_str)).map_err(|e| Error::other(e.to_string()))?;

    // On récupère le message depuis les arguments CLI
    let mut binding = commit::Commit::new();
    let ticket = binding.commit().expect("commit fail");
    let message = binding.to_string();

    vcs::commit(
        &connection,
        message.as_str(),
        author().as_str(),
        ticket.title.as_str(),
    )
    .map_err(|e| Error::other(e.to_string()))?;
    todo::complete_todo(&connection, ticket.id).expect("failed to close todo");

    Ok(())
}
pub fn check_status() -> Result<(), Error> {
    let current_dir = current_dir()?;
    let current_dir_str = current_dir.to_str().unwrap();
    if !Path::new(".lys").exists() {
        return Err(Error::other("Not a lys repository."));
    }

    let connection =
        connect_lys(Path::new(current_dir_str)).map_err(|e| Error::other(e.to_string()))?;
    vcs::status(
        &connection,
        current_dir_str,
        get_current_branch(&connection)
            .expect("failed to get current branch")
            .as_str(),
    )
    .map_err(|e| Error::other(e.to_string()))?;
    Ok(())
}

#[derive(Clone, Ord, Eq, PartialEq, PartialOrd, Debug)]
enum Language {
    Rust,
    Python,
    Haskell,
    CSharp,
    C,
    D,
    Cpp,
    Php,
    Js,
    Typescript,
}
impl Language {
    fn all() -> Vec<Self> {
        let mut x = vec![
            Rust, Python, Haskell, CSharp, C, D, Cpp, Php, Js, Typescript,
        ];
        x.sort_unstable();
        x
    }
    fn get_language_name(&self) -> &'static str {
        match self {
            Rust => "Rust",
            Python => "Python",
            Haskell => "Haskell",
            CSharp => "CSharp",
            C => "C",
            D => "D",
            Cpp => "Cpp",
            Php => "Php",
            Js => "JavaScript",
            Typescript => "TypeScript",
        }
    }
}
impl Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get_language_name())
    }
}

fn generate_readme(project: &str) {
    let mut readme = File::create(format!("{project}{MAIN_SEPARATOR_STR}README.md").as_str())
        .expect("Failed to create README.md file");
    readme
        .write_all(format!("# {project}").as_bytes())
        .expect("failed to generate readme content");
    readme.sync_data().expect("failed to sync");
    ok("readme generated successfully");
}

fn generate_syl(project: &str, to_ignore: Vec<&'static str>) {
    let mut syl = File::create(format!("{project}{MAIN_SEPARATOR_STR}syl").as_str())
        .expect("Failed to create README.md file");
    for x in &to_ignore {
        syl.write_all(format!("{x}\n").as_bytes())
            .expect("failed to save ignore file");
    }
    syl.sync_data().expect("failed to sync");
    ok("syl generated successfully");
}

fn ask(q: &str) -> String {
    let mut resp = String::new();
    while resp.trim().is_empty() {
        resp.clear();
        if let Ok(r) = Text::new(q).prompt() {
            resp = r.to_string();
        }
    }
    resp
}

fn new_project() -> Result<(), Error> {
    let project = ask("Project name:");
    let title = ask("Project title (lysrc):");
    let description = ask("Project description (lysrc):");
    let homepage = ask("Project homepage url (lysrc):");
    let documentation = ask("Project documentation url (lysrc):");

    let email = ask("Email:");
    let author = ask("Username:");
    let supported_languages = Language::all();
    let commiter = format!("{author} <{email}>");

    if connect_lys(Path::new(project.as_str()))
        .expect("failed to get the connexion")
        .execute(LYS_INIT)
        .is_ok()
    {
        let conn = connect_lys(Path::new(project.as_str())).unwrap();
        set_config(&conn, "author", commiter.as_str()).expect("failed to set author");
        set_config(&conn, "name", author.as_str()).expect("failed to set author");
        set_config(&conn, "email", email.as_str()).expect("failed to set email");
        set_config(&conn, "title", title.as_str()).expect("failed to set title");
        set_config(&conn, "description", description.as_str()).expect("failed to set description");
        crypto::generate_keypair(Path::new(project.as_str())).expect("failed to generate keys");
        ok("project keys has been generated successfully");
        let mut lysrc = File::create(format!("{project}{MAIN_SEPARATOR_STR}lysrc").as_str())
            .expect("failed to create file");
        lysrc.write_all(format!("title={title}\ndescription={description}\nlogo=lys.svg\nfavicon=favicon.ico\nfooter=(c) 2026 Lys\nhomepage={homepage}\ndocumentation={documentation}\n").as_bytes()).expect("failed to write lysrc");
        lysrc.sync_all().expect("failed to sync lysrc");
        ok("lysrc file created successfully");
        let main_language = Select::new("Select the main language:", supported_languages)
            .prompt()
            .expect("Failed to select language");
        match main_language {
            D => {
                Cmd::new("dub")
                    .arg("init")
                    .current_dir(project.as_str())
                    .spawn()
                    .expect("Failed to create dub project")
                    .wait()
                    .expect("Failed to wait for dub init");
                generate_readme(project.as_str());
                generate_syl(
                    project.as_str(),
                    vec![
                        "*.exe", "*.lib", "*.dll", "*.dylib", "*.a", "*.o", "*.so", "*.obj",
                        "docs", ".dub",
                    ],
                );
                ok(PROJ_GEN_SUCCESS);
            }
            Rust => {
                let p = Select::new("create a bin or a lib :", vec!["bin", "lib"])
                    .prompt()
                    .expect("Failed to select project type");
                ok(format!("creating a {} project", p.to_lowercase().replace(" ", "")).as_str());
                if p == "bin" {
                    Cmd::new("cargo")
                        .arg("init")
                        .arg("--vcs")
                        .arg("none")
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .current_dir(project.as_str())
                        .spawn()
                        .expect("failed to init cargo project")
                        .wait()
                        .expect("failed to wait");
                } else {
                    Cmd::new("cargo")
                        .arg("init")
                        .arg("--lib")
                        .arg("--vcs")
                        .arg("none")
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .current_dir(project.as_str())
                        .spawn()
                        .expect("Failed to init cargo project")
                        .wait()
                        .expect("failed to wait");
                }
                generate_readme(project.as_str());
                generate_syl(project.as_str(), vec!["target"]);
                ok(PROJ_GEN_SUCCESS);
            }
            Python => {
                Cmd::new("python3")
                    .arg("-m")
                    .arg("venv")
                    .arg(format!("{project}{MAIN_SEPARATOR_STR}.venv"))
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .current_dir(project.as_str())
                    .spawn()
                    .expect("Failed to create .venv")
                    .wait()
                    .expect("Failed to wait for venv creation");
                ok("venv created successfully");

                File::create(format!("{project}{MAIN_SEPARATOR_STR}main.py").as_str())
                    .expect("Failed to create main.py file");
                ok("main.py file created successfully");
                File::create(format!("{project}{MAIN_SEPARATOR_STR}requirements.txt").as_str())
                    .expect("Failed to create requirements file");
                ok("requirements.txt file created successfully");
                generate_readme(project.as_str());
                generate_syl(project.as_str(), vec!["__pycache__", ".venv", ".pyc"]);
                ok(PROJ_GEN_SUCCESS);
            }
            Haskell => {
                Cmd::new("cabal")
                    .arg("init")
                    .current_dir(project.as_str())
                    .spawn()
                    .expect("Failed to create cabal project")
                    .wait()
                    .expect("failed to wait");
                generate_readme(project.as_str());
                generate_syl(project.as_str(), vec![]);
                ok(PROJ_GEN_SUCCESS);
            }
            CSharp => {
                let x = Select::new(
                    "select the project type :",
                    vec!["console", "blazor", "blazor", "wpf", "classlib", "mstest"],
                )
                .prompt()
                .expect("Failed to select project type");
                Cmd::new("dotnet")
                    .arg("new")
                    .arg(x.to_lowercase().replace(" ", ""))
                    .arg("--output")
                    .arg(project.as_str())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .expect("Failed to create dotnet project")
                    .wait()
                    .expect("failed to wait");
                generate_readme(project.as_str());
                generate_syl(project.as_str(), vec![]);
                ok(PROJ_GEN_SUCCESS);
            }
            C => {
                generate_readme(project.as_str());
                let mut cmakelists =
                    File::create(format!("{project}{MAIN_SEPARATOR_STR}CMakeLists.txt").as_str())
                        .expect("Failed to create CMakeLists.txt file");
                cmakelists
                    .write_all(
                        format!("cmake_minimum_required(VERSION 3.10)\nproject({project})\nadd_executable({project} main.c)\n")
                            .as_bytes(),
                    )
                    .expect("faield to write cmakelist");
                cmakelists.sync_data().expect("failed to sync");
                ok("CMakeLists.txt file created successfully");
                generate_syl(
                    project.as_str(),
                    vec![
                        "*o",
                        "*.so",
                        "*a",
                        "*.dll",
                        "build",
                        "CMakeFiles",
                        "MakeFile",
                        "cmake-build-debug",
                        "*.cmake",
                    ],
                );
                ok(PROJ_GEN_SUCCESS);
            }
            Cpp => {
                generate_readme(project.as_str());
                let mut cmakelists =
                    File::create(format!("{project}{MAIN_SEPARATOR_STR}CMakeLists.txt").as_str())
                        .expect("Failed to create CMakeLists.txt file");
                cmakelists
                    .write_all(
                        format!("cmake_minimum_required(VERSION 3.10)\nproject({project})\nadd_executable({project} main.cpp)\n")
                            .as_bytes(),
                    )
                    .expect("faield to write cmakelist");
                cmakelists.sync_data().expect("failed to sync");
                ok("CMakeLists.txt file created successfully");
                generate_syl(
                    project.as_str(),
                    vec![
                        "*o",
                        "*.so",
                        "*a",
                        "*.dll",
                        "build",
                        "CMakeFiles",
                        "MakeFile",
                        "cmake-build-debug",
                        "*.cmake",
                    ],
                );
                ok(PROJ_GEN_SUCCESS);
            }
            Php => {
                Cmd::new("composer")
                    .arg("init")
                    .current_dir(project.as_str())
                    .spawn()
                    .expect("Failed to create laravel project")
                    .wait()
                    .expect("Failed to wait for composer init");
                ok("composer.json created successfully");
                generate_readme(project.as_str());
                generate_syl(project.as_str(), vec!["vendor", "node_modules"]);
                ok(PROJ_GEN_SUCCESS);
            }
            Js => {
                Cmd::new("npm")
                    .arg("init")
                    .current_dir(project.as_str())
                    .spawn()
                    .expect("Failed to create npm project")
                    .wait()
                    .expect("Failed to wait for npm init");
                ok("package.json created successfully");
                generate_readme(project.as_str());
                generate_syl(project.as_str(), vec!["node_modules", "build"]);
                ok(PROJ_GEN_SUCCESS);
            }
            Typescript => {
                Cmd::new("npm")
                    .arg("init")
                    .current_dir(project.as_str())
                    .spawn()
                    .expect("Failed to create npm project")
                    .wait()
                    .expect("Failed to wait for npm init");
                ok("package.json created successfully");
                Cmd::new("tsc")
                    .arg("--init")
                    .current_dir(project.as_str())
                    .spawn()
                    .expect("Failed to init typescript")
                    .wait()
                    .expect("failed to wait");
                ok("tsconfig.json created successfully");
                generate_readme(project.as_str());
                generate_syl(project.as_str(), vec!["node_modules", "build"]);
                ok(PROJ_GEN_SUCCESS);
            }
        }
        Ok(())
    } else {
        Err(Error::other("Failed to create the sqlite database"))
    }
}

fn summary() -> Result<(), Error> {
    let root_path = current_dir().expect("Failed to get current directory");
    let conn = connect_lys(root_path.as_path()).expect("Failed to connect to database");
    let contributors = db::get_unique_contributors(&conn).expect("Failed to get contributors");

    for (contributor, count) in &contributors {
        ok(format!("{contributor} ({count} commits)").as_str());
    }
    Ok(())
}
pub fn execute_matches(app: clap::ArgMatches) -> Result<(), Error> {
    match app.subcommand() {
        Some(("new", _)) => new_project(),
        Some(("email", sub)) => match sub.subcommand() {
            Some(("write", _)) => {
                loop {
                    execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
                    let mut msg = String::new();
                    let to = ask("To:");
                    let subject = ask("Subject:");
                    while msg.is_empty() {
                        msg.clear();
                        msg = Editor::new("Message:")
                            .prompt()
                            .expect("failed to get message");
                    }
                    if msg.trim().is_empty() {
                        ko("Message cannot be empty. Please try again.");
                        continue;
                    }
                    if to.trim().is_empty() {
                        ko("To cannot be empty. Please try again.");
                        continue;
                    }
                    if subject.trim().is_empty() {
                        ko("Subject cannot be empty. Please try again.");
                        continue;
                    }
                    if let Err(_) = email::send(to.as_str(), subject.as_str(), msg.as_str()) {
                        ko("Failed to send email");
                    }
                    if Confirm::new("Do you want to send another email?")
                        .with_default(false)
                        .with_default(false)
                        .prompt()
                        .unwrap_or(false)
                    {
                        continue;
                    } else {
                        break;
                    }
                }
                Ok(())
            }
            Some(("configure", _)) => {
                let from = ask("From (email):");
                let username = ask("SMTP Username:");
                let password = Password::new("SMTP Password:")
                    .prompt()
                    .expect("failed to get password");
                let transport = ask("SMTP Transport (e.g., smtp.gmail.com:587):");
                let port = ask("SMTP Port (e.g., 587):")
                    .parse::<u16>()
                    .expect("failed to parse port");
                email::create_smtp_config(
                    from.as_str(),
                    username.as_str(),
                    password.as_str(),
                    transport.as_str(),
                    port,
                )
                .expect("failed to configure email");
                Ok(())
            }
            Some(("edit", _)) => {
                email::edit_smtp_config().expect("failed to edit smtp config");
                Ok(())
            }
            _ => {
                ko("Invalid email subcommand. Use 'write', 'configure', or 'edit'.");
                Ok(())
            }
        },

        Some(("verify", args)) => {
            let deep = args.get_flag("deep"); // On récupère le flag
            let current_dir = current_dir()?;
            let conn = connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;
            db::verify(&conn, deep).map_err(|e| Error::other(e.to_string()))?;
            Ok(())
        }
        Some(("summary", _)) => summary(),
        Some(("branch", sub)) => {
            let current_dir = current_dir()?;
            let conn = connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;
            match sub.subcommand() {
                Some(("list", _)) => {
                    let all_branches = list_branches(&conn);
                    let current = get_current_branch(&conn).expect("failed to get current branch");
                    let mut table = Builder::new();
                    table.push_record(["Branch", "Updated At", "Summary", "Author", "Email)"]);
                    for branche in all_branches {
                        let br = get_branch_infos(&conn, branche.as_str())
                            .unwrap_or(Some((String::new(), String::new(), String::new())))
                            .expect("failed to get branches infos");
                        let branch = if current == branche {
                            format!("{}*", branche)
                        } else {
                            branche.to_string()
                        };
                        table.push_record([
                            branch.as_str(),
                            time_ago_cli(br.0.as_str()).as_str(),
                            br.2.lines().collect::<Vec<&str>>()[0].to_string().as_str(),
                            br.1.as_str().split("<").collect::<Vec<&str>>()[0],
                            br.1.as_str().split("<").collect::<Vec<&str>>()[1]
                                .replace(">", "")
                                .as_str(),
                        ]);
                    }
                    println!("{}", table.build().with(Style::blank()));
                }
                Some(("delete", sub)) => {
                    let branch = sub.get_one::<String>("branch").expect("branch is required");
                    delete_branch(&conn, branch).expect("faield to delete branch");
                }
                Some(("checkout", sub)) => {
                    let branch = sub.get_one::<String>("branch").expect("branch is required");
                    if list_branches(&conn).contains(branch) {
                        checkout(&conn, branch.as_str()).expect("failed to checkout");
                    } else {
                        ko(format!("{branch} not found").as_str());
                    }
                }
                Some(("create", sub)) => {
                    let branch = sub.get_one::<String>("name").expect("name is required");
                    let is_feat = sub.get_flag("feat");
                    let is_hotfix = sub.get_flag("hotfix");
                    if is_feat {
                        feature_start(&conn, branch.as_str()).expect("failed to create feat");
                    } else if is_hotfix {
                        hotfix_start(&conn, branch.as_str()).expect("failed to create branch");
                    } else {
                        create_branch(&conn, branch.as_str()).expect("failed to create branch");
                    }
                }
                _ => {
                    ok(get_current_branch(&conn)
                        .unwrap_or("main".to_string())
                        .as_str());
                }
            }
            Ok(())
        }
        Some(("prune", _)) => {
            let conn = connect_lys(Path::new(".")).expect("failed to connect to the database");
            let ans = inquire::Confirm::new("Are you sure you want to prune the repository?")
                .with_help_message("This action will PERMANENTLY delete all commits older than 2 years and reclaim disk space.")
                .with_default(false)
                .prompt();
            match ans {
                Ok(true) => {
                    // Lancement de la fonction de nettoyage que nous avons codée
                    db::prune(&conn).expect("failed to prune");
                }
                Ok(false) => println!("Prune operation cancelled."),
                Err(_) => println!("Error during confirmation. Operation aborted."),
            }
            Ok(())
        }
        Some(("serve", args)) => {
            let port: u16 = args
                .get_one::<String>("port")
                .unwrap()
                .parse()
                .unwrap_or(3000);
            let rt = tokio::runtime::Runtime::new()?;
            // On lance le serveur sur le répertoire actuel
            rt.block_on(web::start_server(".", port));
            Ok(())
        }
        Some(("import", sub_m)) => {
            let url = sub_m.get_one::<String>("url").unwrap();
            let depth = sub_m.get_one::<i32>("depth").copied();
            let only_recent = sub_m.get_flag("recent"); // Récupère le flag --recent
            let repo_name = extract_repo_name(url);
            let target_dir = current_dir()?.join(&repo_name);
            // On passe le nouveau paramètre à ta fonction
            import::import_from_git(url, &target_dir, depth, only_recent, false, false)
                .expect("failed");
            ok("ready");
            Ok(())
        }
        Some(("pull", _)) => {
            let current_dir = current_dir()?;
            if !Path::new(".git").exists() {
                return Err(Error::other(
                    "No .git directory found. This is not a Git-backed repo.",
                ));
            }
            let conn = connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;
            let branch = get_current_branch(&conn).map_err(|e| Error::other(e.to_string()))?;
            if branch != "origin" {
                return Err(Error::other(
                    "You must be on the 'origin' branch to pull from Git.",
                ));
            }
            let status_list = vcs::status(&conn, current_dir.to_str().unwrap(), &branch)
                .map_err(|e| Error::other(e.to_string()))?;
            if !status_list.is_empty() {
                return Err(Error::other(
                    "Working tree has changes. Commit or stash before pulling.",
                ));
            }

            let git_status = Cmd::new("git")
                .arg("pull")
                .arg("--ff-only")
                .current_dir(&current_dir)
                .status()?;
            if !git_status.success() {
                return Err(Error::other("git pull failed"));
            }

            let repo =
                git2::Repository::open(&current_dir).map_err(|e| Error::other(e.to_string()))?;
            let head_oid = repo
                .head()
                .ok()
                .and_then(|h| h.target())
                .ok_or_else(|| Error::other("Unable to resolve Git HEAD"))?;
            let head_str = head_oid.to_string();

            let last_oid = {
                let mut stmt = conn
                    .prepare("SELECT value FROM config WHERE key = 'git_origin_head'")
                    .map_err(|e| Error::other(e.to_string()))?;
                if let Ok(State::Row) = stmt.next() {
                    stmt.read::<String, _>("value").ok()
                } else {
                    None
                }
            };

            if let Some(last) = last_oid {
                if last == head_str {
                    ok("Already up to date.");
                    return Ok(());
                }
                import::import_updates_from_repo(&current_dir, &current_dir, &last, "origin")
                    .map_err(|e| Error::other(e.to_string()))?;
            } else {
                ok("No previous Git head found. Recording current HEAD.");
            }

            let mut stmt = conn
                .prepare(
                    "INSERT INTO config (key, value) VALUES ('git_origin_head', ?) \
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .map_err(|e| Error::other(e.to_string()))?;
            stmt.bind((1, head_str.as_str()))
                .map_err(|e| Error::other(e.to_string()))?;
            stmt.next().map_err(|e| Error::other(e.to_string()))?;
            ok("Git pull + Lys sync complete.");
            Ok(())
        }
        Some(("mount", sub_args)) => {
            let target = sub_args.get_one::<String>("target").unwrap();
            let reference = sub_args.get_one::<String>("ref");
            let current_dir = current_dir()?;
            let conn = connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;
            vcs::mount_version(&conn, target, reference.map(|s| s.as_str()))
                .map_err(|e| Error::other(e.to_string()))
        }
        Some(("shell", sub_args)) => {
            let reference = sub_args.get_one::<String>("ref").map(|s| s.as_str());
            let current_dir = current_dir()?;
            let conn = connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;
            vcs::spawn_lys_shell(&conn, reference).map_err(|e| Error::other(e.to_string()))
        }
        Some(("init", _)) => {
            let current_dir = current_dir()?;
            let path_str = current_dir.to_str().unwrap();
            if connect_lys(Path::new(path_str))
                .expect("fail")
                .execute(LYS_INIT)
                .is_ok()
            {
                generate_keypair(Path::new(path_str)).expect("failed to generate keys");
                ok("Initialized empty lys repository");
                Ok(())
            } else {
                Err(Error::other("Failed to init"))
            }
        }
        Some(("clone", args)) => {
            let url = args.get_one::<String>("url").unwrap();

            // Récupération de la depth
            let depth = args.get_one::<i32>("depth").copied();

            // 1. Déterminer le nom du dossier (soit l'arg, soit déduit de l'URL)
            let dir_name = if let Some(name) = args.get_one::<String>("name") {
                name.clone()
            } else {
                extract_repo_name(url)
            };

            let target_path = current_dir()?.join(&dir_name);

            // 2. Vérifier si ça existe déjà pour ne pas écraser
            if target_path.exists() {
                ko("already exist");
                return Ok(());
            }

            // 3. Cloner et importer en conservant .git
            ok("Creation of the repository");
            import::import_from_git(url, &target_path, depth, false, true, true)
                .expect("failed to clone");
            ok("ready");
            Ok(())
        }
        Some(("tree", _)) => {
            let current_dir = current_dir()?;
            let conn = connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;

            // 1. On récupère la branche actuelle
            let branch = get_current_branch(&conn).expect("failed to get branch");

            // 2. On récupère le tree_hash associé au HEAD de cette branche
            let query = "SELECT c.id, c.tree_hash FROM branches b JOIN commits c ON b.head_commit_id = c.id WHERE b.name = ?";
            let mut stmt = conn
                .prepare(query)
                .map_err(|e| Error::other(e.to_string()))?;
            stmt.bind((1, branch.as_str())).unwrap();

            if let Ok(State::Row) = stmt.next() {
                let head_commit_id: i64 = stmt.read(0).unwrap();
                let root_hash: String = stmt.read(1).unwrap();
                ok_merkle_hash(root_hash.as_str());
                let lines = vcs::ls_tree(&conn, &root_hash, "", head_commit_id)
                    .map_err(|e| Error::other(e.to_string()))?;

                if let Some(mut child) = vcs::start_pager() {
                    if let Some(mut stdin) = child.stdin.take() {
                        let output = lines.join("\n");
                        let _ = stdin.write_all(output.as_bytes());
                        drop(stdin);
                        let _ = child.wait();
                    } else {
                        println!("{}", lines.join("\n"));
                    }
                } else {
                    println!("{}", lines.join("\n"));
                }
            } else {
                ok("repository empty. Commit something first!");
            }
            Ok(())
        }
        Some(("keygen", _)) => {
            let current_dir = current_dir()?;
            crypto::generate_keypair(&current_dir).expect("failed to create keys");
            ok("keys generated successfully");
            Ok(())
        }
        Some(("status", _)) => check_status(),
        Some(("chat", sub)) => {
            let sender = std::env::var("USER").expect("USER must be defined");
            let conn = connect_lys(Path::new(".")).expect("failed to connect to the database");
            match sub.subcommand() {
                Some(("write", _arg)) => {
                    loop {
                        let mut msg = String::new();
                        while msg.is_empty() {
                            msg.clear();
                            msg = Editor::new("message to send:")
                                .prompt()
                                .expect("message it's required");
                        }
                        if Confirm::new(msg.as_str())
                            .with_default(true)
                            .prompt()
                            .expect("failed to confirm")
                        {
                            send_message(&conn, sender.as_str(), msg.as_str())
                                .expect("failed to ssend message");
                        }
                        if Confirm::new("continue")
                            .with_default(false)
                            .prompt()
                            .expect("failed to get confirm")
                            .eq(&false)
                        {
                            break;
                        }
                    }
                    Ok(())
                }
                Some(("list", _)) => match list_messages(&conn) {
                    Ok(messages) => {
                        if messages.is_empty() {
                            ok("chat messages is empty.");
                            Ok(())
                        } else {
                            // 1. On génère la grosse String formatée avec tous tes writeln!
                            let full_content = messages
                                .iter()
                                .map(|m| m.to_string())
                                .collect::<Vec<String>>()
                                .join("");
                            internal_pager(full_content).expect("failed to pager");
                            Ok(())
                        }
                    }
                    Err(_) => Err(Error::other("Failed to read messages")),
                },
                _ => Ok(()),
            }
        }
        Some(("audit", _)) => {
            let conn = connect_lys(Path::new(".")).expect("failed to connect to the databaase");
            if crypto::audit(&conn).expect("failed to connect to the database") {
                Ok(())
            } else {
                Err(Error::other("audit detect failure"))
            }
        }
        Some(("health", _)) => {
            if run_hooks().is_ok() {
                ok("code can be commited");
            } else {
                ko("code must not be commited");
            }
            Ok(())
        }
        Some(("commit", _)) => {
            if read_to_string("syl")
                .expect("missing syl file")
                .trim()
                .is_empty()
            {
                return Err(Error::other(
                    "syl content cannot be empty ignore content before commit.",
                ));
            }
            perform_commit()
        }
        Some(("log", args)) => {
            let page = *args.get_one::<usize>("page").unwrap();
            let limit = *args.get_one::<usize>("limit").unwrap();
            let conn = connect_lys(Path::new(".")).expect("failed to connect to the database");
            vcs::log(&conn, page, limit).expect("failed to parse log");
            Ok(())
        }
        Some(("diff", _)) => {
            let current_dir = current_dir()?;
            let conn =
                connect_lys(current_dir.as_path()).map_err(|e| Error::other(e.to_string()))?;
            vcs::diff(&conn).map_err(|e| Error::other(e.to_string()))
        }
        Some(("restore", sub_matches)) => {
            let current_dir = current_dir()?;
            let conn =
                connect_lys(current_dir.as_path()).map_err(|e| Error::other(e.to_string()))?;

            let path = sub_matches.get_one::<String>("path").unwrap();
            vcs::restore(&conn, path).map_err(|e| Error::other(e.to_string()))
        }
        Some(("switch", sub_matches)) => {
            let current_dir = current_dir()?;
            let conn =
                connect_lys(current_dir.as_path()).map_err(|e| Error::other(e.to_string()))?;
            let name = sub_matches.get_one::<String>("branch").unwrap();
            vcs::checkout(&conn, name).map_err(|e| Error::other(e.to_string()))
        }
        Some(("feat", sub_matches)) => {
            let current_dir = current_dir()?;
            let conn =
                connect_lys(current_dir.as_path()).map_err(|e| Error::other(e.to_string()))?;

            match sub_matches.subcommand() {
                Some(("start", args)) => {
                    let name = args.get_one::<String>("name").unwrap();
                    feature_start(&conn, name).map_err(|e| Error::other(e.to_string()))
                }
                Some(("finish", args)) => {
                    let name = args.get_one::<String>("name").unwrap();
                    feature_finish(&conn, name).map_err(|e| Error::other(e.to_string()))
                }
                _ => {
                    ok("Please specify 'start' or 'finish'.");
                    Ok(())
                }
            }
        }
        Some(("hotfix", sub_matches)) => {
            let current_dir = current_dir()?;
            let conn =
                connect_lys(current_dir.as_path()).map_err(|e| Error::other(e.to_string()))?;

            match sub_matches.subcommand() {
                Some(("start", args)) => {
                    let name = args.get_one::<String>("name").unwrap();
                    hotfix_start(&conn, name).map_err(|e| Error::other(e.to_string()))
                }
                Some(("finish", args)) => {
                    let name = args.get_one::<String>("name").unwrap();
                    hotfix_finish(&conn, name).map_err(|e| Error::other(e.to_string()))
                }
                _ => {
                    ok("please specify 'start' or 'finish'");
                    Ok(())
                }
            }
        }
        Some(("tag", sub_matches)) => {
            let current_dir = current_dir()?;
            let conn =
                connect_lys(current_dir.as_path()).map_err(|e| Error::other(e.to_string()))?;

            match sub_matches.subcommand() {
                Some(("audit", _)) => audit_tags(),
                Some(("create", args)) => {
                    let name = args.get_one::<String>("name").unwrap();
                    let msg = args.get_one::<String>("message").map(|s| s.as_str());
                    tag_create(&conn, name, msg)
                }
                Some(("list", _)) => tag_list(&conn),
                _ => {
                    ok("Please use 'create' or 'list'.");
                    Ok(())
                }
            }
        }
        Some(("backup", args)) => {
            let current_dir = current_dir()?;
            let _conn =
                connect_lys(current_dir.as_path()).map_err(|e| Error::other(e.to_string()))?;
            let path = args.get_one::<String>("path").unwrap();
            vcs::sync(path)
        }
        Some(("spotify", args)) => {
            let url = args.get_one::<String>("url").unwrap();
            let current_dir = current_dir()?;
            let conn = connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;
            let mut stmt = conn
                .prepare("INSERT OR REPLACE INTO config (key, value) VALUES ('spotify_url', ?)")
                .map_err(|e| Error::other(e.to_string()))?;
            stmt.bind((1, url.as_str()))
                .map_err(|e| Error::other(e.to_string()))?;
            stmt.next().map_err(|e| Error::other(e.to_string()))?;
            ok("Music URL updated for web interface");
            Ok(())
        }
        Some(("video", args)) => {
            let url = args.get_one::<String>("url").unwrap();
            let current_dir = current_dir()?;
            let conn = connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;
            let mut stmt = conn
                .prepare(
                    "INSERT OR REPLACE INTO config (key, value) VALUES ('video_banner_url', ?)",
                )
                .map_err(|e| Error::other(e.to_string()))?;
            stmt.bind((1, url.as_str()))
                .map_err(|e| Error::other(e.to_string()))?;
            stmt.next().map_err(|e| Error::other(e.to_string()))?;
            ok("Video banner URL updated for web interface");
            Ok(())
        }
        Some(("banner", args)) => {
            let url = args.get_one::<String>("url").unwrap();
            let current_dir = current_dir()?;
            let conn = connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;
            let mut stmt = conn
                .prepare("INSERT OR REPLACE INTO config (key, value) VALUES ('banner_url', ?)")
                .map_err(|e| Error::other(e.to_string()))?;
            stmt.bind((1, url.as_str()))
                .map_err(|e| Error::other(e.to_string()))?;
            stmt.next().map_err(|e| Error::other(e.to_string()))?;
            ok("Image banner URL updated for web interface");
            Ok(())
        }
        Some(("todo", sub)) => {
            let current_dir = current_dir()?;
            let conn =
                connect_lys(current_dir.as_path()).expect("failed to connect to the database");
            match sub.subcommand() {
                Some(("add", args)) => {
                    // If a title (or other args) was provided, handle non-interactive add
                    if let Some(title) = args.get_one::<String>("title") {
                        let conn =
                            connect_lys(Path::new(".")).expect("failed to connect to the db");
                        let description = args
                            .get_one::<String>("description")
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        let assigned_to = args
                            .get_one::<String>("user")
                            .map(|s| s.as_str())
                            .unwrap_or("me");
                        let due_date = args
                            .get_one::<String>("due")
                            .map(|s| s.as_str())
                            .unwrap_or("No limit");

                        todo::add_todo(&conn, title.as_str(), description, assigned_to, due_date)
                            .map_err(|e| Error::other(e.to_string()))?;
                        ok(format!(
                            "Todo added: {} (assigned to: {}, due: {})",
                            title, assigned_to, due_date
                        )
                        .as_str());
                        return Ok(());
                    }

                    // Fallback to interactive mode when no title arg
                    loop {
                        execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
                        let title = Text::new("Title:").prompt().expect("failed to get title");
                        let description = Editor::new("Description:")
                            .prompt()
                            .expect("failed to get todo description");
                        let conn =
                            connect_lys(Path::new(".")).expect("faield to cconnect to the db");
                        let users = db::get_unique_contributors(&conn)
                            .unwrap_or(Vec::from([(String::from("me"), 1)]));
                        let user = Select::new(
                            "Assign to:",
                            users.iter().map(|(u, _)| u).collect::<Vec<&String>>(),
                        )
                        .prompt()
                        .expect("failed to get asigned to ");
                        let date = DateSelect::new("Due date:")
                            .prompt()
                            .expect("failed to get due date");

                        if todo::add_todo(
                            &conn,
                            title.as_str(),
                            &description,
                            user.as_str(),
                            date.to_string().as_str(),
                        )
                        .is_ok()
                            && Confirm::new("add an other todo?")
                                .with_default(true)
                                .prompt()
                                .expect("value is required")
                                .eq(&false)
                        {
                            ok("bye");
                            break;
                        }
                    }
                    Ok(())
                }
                Some(("start", args)) => {
                    let id = args.get_one::<i64>("id").unwrap();
                    todo::start_todo(&conn, *id).expect("failed to start todo");
                    Ok(())
                }
                Some(("list", _)) => {
                    todo::list_todos(&conn).map_err(|e| Error::other(e.to_string()))
                }
                Some(("close", args)) => {
                    let id = args.get_one::<i64>("id").unwrap();
                    todo::complete_todo(&conn, *id).expect("failed to complete todo");
                    Ok(())
                }
                _ => Ok(()),
            }
        }
        Some(("web", sub)) => {
            match sub.subcommand() {
                Some(("edit", _)) => {
                    let editor = std::env::var("EDITOR").unwrap_or("vi".to_string());
                    std::process::Command::new(editor)
                        .arg("lys.yml")
                        .current_dir(".")
                        .spawn()
                        .expect("editor missing")
                        .wait()
                        .expect("failed to wait");
                    ok("bye");
                    Ok(())
                }
                Some(("info", _)) => {
                    let lysrc: Lysrc = serde_yaml::from_reader(
                        File::open(Path::new("lys.yml")).expect("lys.yml not found"),
                    )
                    .expect("failed to parse lys.yml");

                    let lys = Table::new([lysrc]);
                    println!("{lys}");
                    Ok(())
                }
                Some(("run", args)) => {
                    let current_dir = current_dir()?;
                    let current_dir_str = current_dir.to_str().unwrap();
                    if !Path::new(".lys").exists() {
                        return Err(Error::other("Not a lys repository."));
                    }

                    let conn =
                        connect_lys(&current_dir).map_err(|e| Error::other(e.to_string()))?;

                    if let Some(spotify_url) = args.get_one::<String>("spotify") {
                        let mut stmt = conn
                    .prepare("INSERT OR REPLACE INTO config (key, value) VALUES ('spotify_url', ?)")
                    .map_err(|e| Error::other(e.to_string()))?;
                        stmt.bind((1, spotify_url.as_str()))
                            .map_err(|e| Error::other(e.to_string()))?;
                        stmt.next().map_err(|e| Error::other(e.to_string()))?;
                        ok("Music URL updated");
                    }

                    if let Some(video_url) = args.get_one::<String>("video") {
                        let mut stmt = conn
                    .prepare(
                        "INSERT OR REPLACE INTO config (key, value) VALUES ('video_banner_url', ?)",
                    )
                    .map_err(|e| Error::other(e.to_string()))?;
                        stmt.bind((1, video_url.as_str()))
                            .map_err(|e| Error::other(e.to_string()))?;
                        stmt.next().map_err(|e| Error::other(e.to_string()))?;
                        ok("Video banner URL updated");
                    }

                    if let Some(banner_url) = args.get_one::<String>("banner") {
                        let mut stmt = conn
                    .prepare("INSERT OR REPLACE INTO config (key, value) VALUES ('banner_url', ?)")
                    .map_err(|e| Error::other(e.to_string()))?;
                        stmt.bind((1, banner_url.as_str()))
                            .map_err(|e| Error::other(e.to_string()))?;
                        stmt.next().map_err(|e| Error::other(e.to_string()))?;
                        ok("Image banner URL updated");
                    }

                    if let Some(title) = args.get_one::<String>("title") {
                        let mut stmt = conn
                    .prepare("INSERT OR REPLACE INTO config (key, value) VALUES ('web_title', ?)")
                    .map_err(|e| Error::other(e.to_string()))?;
                        stmt.bind((1, title.as_str()))
                            .map_err(|e| Error::other(e.to_string()))?;
                        stmt.next().map_err(|e| Error::other(e.to_string()))?;
                        ok("Web title updated");
                    }

                    if let Some(subtitle) = args.get_one::<String>("subtitle") {
                        let mut stmt = conn
                    .prepare(
                        "INSERT OR REPLACE INTO config (key, value) VALUES ('web_subtitle', ?)",
                    )
                    .map_err(|e| Error::other(e.to_string()))?;
                        stmt.bind((1, subtitle.as_str()))
                            .map_err(|e| Error::other(e.to_string()))?;
                        stmt.next().map_err(|e| Error::other(e.to_string()))?;
                        ok("Web subtitle updated");
                    }

                    if let Some(footer) = args.get_one::<String>("footer") {
                        let footer_content =
                            if Path::new(footer).exists() && Path::new(footer).is_file() {
                                read_to_string(footer).unwrap_or_else(|_| footer.clone())
                            } else {
                                footer.clone()
                            };
                        let mut stmt = conn
                    .prepare("INSERT OR REPLACE INTO config (key, value) VALUES ('web_footer', ?)")
                    .map_err(|e| Error::other(e.to_string()))?;
                        stmt.bind((1, footer_content.as_str()))
                            .map_err(|e| Error::other(e.to_string()))?;
                        stmt.next().map_err(|e| Error::other(e.to_string()))?;
                        ok("Web footer updated");
                    }

                    if let Some(homepage) = args.get_one::<String>("homepage") {
                        let mut stmt = conn
                    .prepare(
                        "INSERT OR REPLACE INTO config (key, value) VALUES ('web_homepage', ?)",
                    )
                    .map_err(|e| Error::other(e.to_string()))?;
                        stmt.bind((1, homepage.as_str()))
                            .map_err(|e| Error::other(e.to_string()))?;
                        stmt.next().map_err(|e| Error::other(e.to_string()))?;
                        ok("Web homepage URL updated");
                    }

                    if let Some(documentation) = args.get_one::<String>("documentation") {
                        let mut stmt = conn
                    .prepare("INSERT OR REPLACE INTO config (key, value) VALUES ('web_documentation', ?)")
                    .map_err(|e| Error::other(e.to_string()))?;
                        stmt.bind((1, documentation.as_str()))
                            .map_err(|e| Error::other(e.to_string()))?;
                        stmt.next().map_err(|e| Error::other(e.to_string()))?;
                        ok("Web documentation URL updated");
                    }

                    let port: u16 = args
                        .get_one::<String>("port")
                        .unwrap()
                        .parse()
                        .unwrap_or(3000);
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(web::start_server(current_dir_str, port));
                    Ok(())
                }
                _ => {
                    cli().print_help().expect("failed to print help");
                    Ok(())
                }
            }
        }
        _ => {
            if Path::new(".lys").is_dir() {
                Shell::new().run()
            } else {
                ko("not a lys repository");
                Ok(())
            }
        }
    }
}

fn main() -> Result<(), Error> {
    dotenv().ok();
    let args = cli();
    let app = args.clone().try_get_matches();
    match app {
        Ok(matches) => execute_matches(matches),
        Err(e) => {
            if std::env::args().len() == 1 {
                Shell::new().run().map_err(|e| Error::other(e.to_string()))
            } else {
                e.exit();
            }
        }
    }
}
