# Fish completion for awq
#  
# Variables
set -l commands init new email web branch verify summary status push pull prune shell mount tree import keygen serve audit log diff clone health todo commit restore chat backup switch feat hotfix tag spotify video banner help

# Disable file completion by default
complete -c awq -f

# Global options
complete -c awq -n "not __fish_seen_subcommand_from $commands" -s h -l help -d 'Print help'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -s V -l version -d 'Print version'

# Commands
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a init -d 'Initialize the current directory'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a new -d 'Create a new awq project'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a verify -d 'Check repository integrity'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a serve -d 'Start the Silex Node'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a log -d 'Show commit logs'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a web -d 'Run or get web info'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a todo -d 'Manage project todos'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a feat -d 'Manage feature branches'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a hotfix -d 'Manage hotfix branches'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a chat -d 'Chat with the team'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a tag -d 'Manage version tags'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a email -d 'Write and send email'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a branch -d 'Branch managment'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a summary -d 'Show working directory infos'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a status -d 'Show changes in working directory'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a push -d 'Push local commits to a remote'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a pull -d 'Pull commits from a remote'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a prune -d 'Maintain repository health by removing old history'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a shell -d 'Open a temporary shell with the code mounted'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a mount -d 'Mount a specific version or the current head'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a tree -d 'Show repository merkle tree'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a keygen -d 'Generate Ed25519 identity keys for signing commits'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a audit -d 'Verify integrity of commit signatures'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a import -d 'Import a Git repository into Lys'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a clone -d 'Clone a Git repository into a new awq repository'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a diff -d 'Show changes between working tree and last commit'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a health -d 'Verify the source code'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a commit -d 'Record change to the repository'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a restore -d 'Discard change to the repository'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a switch -d 'Switch branches'
complete -c awq -n "not __fish_seen_subcommand_from $commands" -a backup -d 'Backup changes'
# (Ajoute les autres descriptions ici si tu le souhaites)
for cmd in spotify video banner
    complete -c awq -n "not __fish_seen_subcommand_from $commands" -a $cmd
end

# Specific options for subcommands
complete -c awq -n "__fish_seen_subcommand_from verify" -l deep -d 'Recalculate blake3 checksums'
complete -c awq -n "__fish_seen_subcommand_from log" -s p -d 'Page number' -r
complete -c awq -n "__fish_seen_subcommand_from log" -s n -l limit -d 'Limit count' -r
complete -c awq -n "__fish_seen_subcommand_from serve" -s p -d Port -r
complete -c awq -n "__fish_seen_subcommand_from web" -s p -d Port -r
complete -c awq -n "__fish_seen_subcommand_from web" -l spotify -l video -l banner -l title -l subtitle

# Subcommands for feature/hotfix/todo etc.
complete -c awq -n "__fish_seen_subcommand_from todo" -a "add start list close"
complete -c awq -n "__fish_seen_subcommand_from feat" -a "start finish"
complete -c awq -n "__fish_seen_subcommand_from hotfix" -a "start finish"
complete -c awq -n "__fish_seen_subcommand_from chat" -a "send list"
complete -c awq -n "__fish_seen_subcommand_from tag" -a "create list audit"
