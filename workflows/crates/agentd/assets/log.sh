# agentd's standard logger for script stages. Source it, then log:
#
#   . "$AGENTD_LOG_SH"
#   log_step  "triggering superblocks/linear-context"
#   log_info  "wrote 6 files"
#   log_ok    "done"
#   log_warn  "gh could not read the PR — keeping it on the list"
#   log_error "no ticket given"
#
# Every line comes out as `LEVEL source: message`, the format the log views
# read exactly: the level sets the colour, the source is shown apart from
# the message. Source is the agent's name; set AGENTD_LOG_SOURCE to say
# something narrower (gather.sh is the `gather` part of `linear`).
#
# warn and error go to stderr, like any well-behaved tool; agentd captures
# both streams into the same log, in order.
#
# Written by agentd on startup — do not edit this copy; it is replaced.

_agentd_log() {
    _lvl=$1
    shift
    printf '%s %s: %s\n' "$_lvl" "${AGENTD_LOG_SOURCE:-${AGENTD_AGENT:-script}}" "$*"
}
log_debug() { _agentd_log DEBUG "$@"; }
log_info()  { _agentd_log INFO "$@"; }
log_step()  { _agentd_log STEP "$@"; }
log_ok()    { _agentd_log OK "$@"; }
log_warn()  { _agentd_log WARN "$@" >&2; }
log_error() { _agentd_log ERROR "$@" >&2; }
