# awq

> AWQ is an experimental version control system / storage engine written in Rust. Currently under heavy active development. Not ready for production use.

[![Rust](https://github.com/kognitara/awq/actions/workflows/rust.yml/badge.svg)](https://github.com/kognitara/awq/actions/workflows/rust.yml)

# AWQ (Advanced Workspace & Query)

**AWQ** (formerly known as *Lys*) is a next-generation, SQLite-backed Version Control System (VCS) and project management workspace written in Rust.

Unlike traditional VCS tools that rely heavily on flat files, AWQ leverages the power of relational databases (SQLite) and cryptographic hashing (Blake3) to provide an immutable, lightning-fast, and verifiable Merkle tree of your project's history. It seamlessly integrates version control, task management (todos), team communication (chat), and web serving into a single unified CLI.

## Key Features

* **Relational VCS Engine:** Blobs, commits, and trees are stored as zlib-compressed data within an embedded SQLite database (`.lys/db/store.db`). This allows for complex historical queries in milliseconds.
* **Cryptographic Integrity:** Powered by **Blake3**, AWQ provides extreme performance for calculating file hashes and building the Merkle Tree root. Every commit is cryptographically signed (Ed25519) to ensure total auditability.
* **Interactive TUI & Visual Logs:** Say goodbye to dry terminal outputs. AWQ features a rich, colorful, and heavily icon-driven interface. Commands like `awq tree` and `awq log` offer paginated, highly readable views of your project's state.
* **Space-Themed Commit Semantics:** Committing code is guided through an interactive prompt categorizing changes into logical "Space" themes (e.g., *Star* for features, *Comet* for bug fixes, *Nebula* for refactors) enforcing a clean and readable project history.
* **Git Interoperability:** Need to work with the outside world? AWQ can clone, pull, and push to remote Git repositories natively while maintaining its own SQLite-based state.
* **Virtual Mounting:** Temporarily mount any specific commit or branch to a local directory (`awq mount`) or drop into an ephemeral shell to test an old state (`awq shell`), without altering your current working directory.
* **Built-in Workspace Tools:** Manage your tasks (`awq todo`), communicate with collaborators (`awq chat`, `awq email`), or instantly scaffold new projects in 10+ languages (`awq new`).

## Installation

Ensure you have the Rust toolchain installed. Clone the repository and build from source:

```bash
cargo install awq
```

* Note: AWQ requires a terminal with Nerd Fonts installed (e.g., JetBrainsMono Nerd Font) to correctly render the UI icons.*

## Quick Start

### 1. Initialize a new project

You can either initialize an existing directory or use the scaffolding tool to create a new one:

```bash
# Scaffold a new project (Rust, Python, C, C++, JS, etc.)
awq new

# OR Initialize an existing directory
awq init
```

### 2. Make your first commit

AWQ uses an interactive prompt to guide you through creating highly structured commit messages.

```bash
awq commit
```

You will be prompted to select a category, a ticket/todo, and provide a summary of *What*, *Why*, and *How* you made the changes.

### 3. Explore your repository

Visualize your current working directory, including the Merkle Root Hash and the last commit for every file:

```bash
awq tree
```

View the detailed, paginated commit history with line addition/deletion statistics:

```bash
awq log
```

## Architecture

AWQ uses a 3-tier hash system displayed natively in the UI:

1. **Merkle Root Hash:** The global fingerprint of the entire repository at a given state.
2. **Commit Hash:** The unique identifier of the historical event (Author, Date, Message, Parent).
3. **Blob Hash:** The Blake3 hash of the actual file content, guaranteeing data integrity.

Because AWQ is backed by SQLite, features like `awq prune` (cleaning old history to reclaim disk space) or querying specific file states across thousands of commits are heavily optimized and native.

## Contributing

Contributions, issues, and feature requests are welcome! Feel free to check the issues page.
