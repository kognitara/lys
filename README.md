# Lys

ˋˋˋtsch
# tcsh completion for lys

set _lys_cmds = (init new email web branch verify summary status push pull prune shell mount tree import keygen serve audit log diff clone health todo commit restore chat sync checkout feat hotfix tag spotify video banner help)

complete lys \
    'c/-/(h help V version)/' \
    'p/1/($_lys_cmds)/' \
    'n/verify/(--deep)/' \
    'n/log/(-p -n --limit)/' \
    'n/serve/(-p)/' \
    'n/web/(-p --spotify --video --banner --title --subtitle --footer --homepage --documentation)/' \
    'n/todo/(add start list close)/' \
    'n/feat/(start finish)/' \
    'n/hotfix/(start finish)/' \
    'n/chat/(send list)/' \
    'n/tag/(create list)/'
ˋˋˋ
