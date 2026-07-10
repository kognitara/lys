# awq

> AWQ is an experimental version control system / storage engine written in Rust. Currently under heavy active development. Not ready for production use.

[![Rust](https://github.com/kognitara/awq/actions/workflows/rust.yml/badge.svg)](https://github.com/kognitara/awq/actions/workflows/rust.yml)

## The AWQ Commit Philosophy: A Developer's Guide

Welcome to the AWQ version control system.

Unlike traditional VCS workflows that allow quick, one-line commit messages, AWQ introduces intentional friction into the commit process. We believe that while your code tells the machine *what* to do, your commit history must tell future engineers *why* it was done. A commit is not just a save point; it is architectural documentation.

When you run `awq commit`, you will be guided through a series of prompts. Here is how to answer them effectively:

### 1. The Ticket (To-Do Resolution)

AWQ bridges the gap between project management and version control. Before categorizing your work, you must link the commit to an existing To-Do item.

* **Rule:** You cannot commit "floating" code. Every change must resolve or contribute to an identified task, bug, or feature in the local database.

### 2. The Category & Space Taxonomy

AWQ uses a unique astronomical taxonomy to categorize changes instantly. Choose the event that best matches your work:

* **Big Bang:** Initializing a new repository or major architectural scaffolding.
* **Star:** Adding or refining a shiny new feature.
* **Asteroid Belt:** Sweeping up, code cleanup, and general maintenance.
* **Quantum Fluctuation:** Tiny, unpredictable, but necessary modifications (e.g., typos, comments).
* *(Select the one that best fits the scale and intent of your change).*

### 3. The Summary (Git Compatibility)

Because AWQ seamlessly syncs with standard Git objects, your summary must respect standard Git conventions.

* **Rule:** Keep it under 50 characters. Use an imperative action verb.
* **Bad:** `fixed the padding issue on the cli`
* **Good:** `Enforce 7-character padding on CLI output`

### 4. What? (The Objective)

Define the exact scope of your modification without diving into the code itself.

* **Formula:** Action verb + Target Component.
* **Example:** "Implement automatic Git synchronization and route standard hook outputs to `/dev/null`."

### 5. Why? (The Architectural Intent)

This is the most critical field. Code cannot explain business logic, future-proofing, or avoided technical debt. Answer this question: *What fails or becomes painful if this commit does not exist?*

* **Example:** "Maintaining interoperability with standard Git tooling is critical for external CI/CD pipelines. Silencing hooks prevents terminal visual pollution."

### 6. How? (The Mechanics)

Provide a high-level summary of the execution strategy. Save the reviewer from reading a 500-line diff to understand your approach.

* **Formula:** I used [Tool/Logic/Crate] to modify [Target].
* **Example:** "Integrated the `git2` crate to mirror AWQ index states. Updated format macros using `{:^7}`."

### 7. Outcome (The Immediate Result)

State the concrete benefit of this code being merged right now.

* **Example:** "A cleaner, Unix-style aligned terminal interface and a robust dual-write version control workflow."

### 8. Breaking Changes?

Be explicit. If this change requires downstream users or other developers to update their configurations, list those requirements here. If not, a simple "None." is perfect.

**Remember:** Take your time. A well-crafted AWQ commit preserves the historical context of the project forever.
