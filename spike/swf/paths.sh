# Sourced by the spike's scripts. The extracted Interface files and the
# harness's output (disassembly, captures) are game assets or derived from
# them, so they live outside the repository:
#   $SPIKE_SWF_HOME/interface   extract.sh's menus, font libraries and text
#   $SPIKE_SWF_HOME/out         disassembly, folded ActionScript, PNGs, logs
# SPIKE_SWF_HOME defaults to ${XDG_CACHE_HOME:-~/.cache}/vibe_rm/swf.
SPIKE_SWF_HOME="${SPIKE_SWF_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/vibe_rm/swf}"
case "$SPIKE_SWF_HOME" in
    "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"/*)
        echo "SPIKE_SWF_HOME must be outside the repository" >&2
        exit 1
        ;;
esac
SPIKE_INTERFACE="$SPIKE_SWF_HOME/interface"
SPIKE_OUT="$SPIKE_SWF_HOME/out"
