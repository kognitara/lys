use std::process::{Command, Stdio};

use unic_langid::LanguageIdentifier;

use crate::vcs::{
    hooks::{HookItem, group_hooks},
    ko, locale, ok, tt,
};

fn run(locale: &LanguageIdentifier, hook: &HookItem) {
    if hook.is_active {
        ok(format!("Running : {} for {} event", hook.name, hook.trigger).as_str());
        if Command::new("sh")
            .arg("-c")
            .arg(hook.command.as_str())
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .current_dir(".")
            .spawn()
            .expect("bad command")
            .wait()
            .expect("failed to wait")
            .success()
        {
            ok(tt(locale, "hook-success").as_str());
        } else {
            ok(tt(locale, "hook-failure").as_str());
        }
    } else {
        ko(format!(
            "The hook : {} for {} event is not active",
            hook.name, hook.trigger
        )
        .as_str());
    }
}
pub async fn check() -> bool {
    let pre_commit = group_hooks(crate::vcs::hooks::Trigger::PreCommit).await;
    let post_commit = group_hooks(crate::vcs::hooks::Trigger::PostCommit).await;
    let pre_push = group_hooks(crate::vcs::hooks::Trigger::PrePush).await;
    let post_push = group_hooks(crate::vcs::hooks::Trigger::PostPush).await;
    let lang = locale();
    if pre_commit.is_empty() {
        ko(tt(&lang, "warning-pre-commit-hooks-empty").as_str());
    } else {
        ok(tt(&lang, "pre-commit-hooks-not-empty").as_str());
    }

    if post_commit.is_empty() {
        ko(tt(&lang, "warning-post-commit-hooks-empty").as_str());
    } else {
        ok(tt(&lang, "post-commit-hooks-not-empty").as_str());
    }
    if pre_push.is_empty() {
        ko(tt(&lang, "warning-pre-push-hooks-empty").as_str());
    } else {
        ok(tt(&lang, "pre-push-hooks-not-empty").as_str());
    }
    if post_push.is_empty() {
        ko(tt(&lang, "warning-post-push-hooks-empty").as_str());
    } else {
        ok(tt(&lang, "post-push-hooks-not-empty").as_str());
    }
    for hook in &pre_commit {
        run(&lang, hook);
    }
    for hook in &post_commit {
        run(&lang, hook);
    }
    for hook in &pre_push {
        run(&lang, hook);
    }
    for hook in &post_push {
        run(&lang, hook);
    }
    true
}
