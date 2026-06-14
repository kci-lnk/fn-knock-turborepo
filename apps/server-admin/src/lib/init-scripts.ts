import { writeFileSync, chmodSync } from "node:fs";
import { join } from "node:path";
import { dataPath } from "./AppDirManager";

const CLEAN_SCRIPT_CONTENT = `#!/bin/bash

CHAINS=("FN-KNOCK-FW" "FN-KNOCK-SSH")
PARENTS=("INPUT" "DOCKER-USER")
TABLES=("iptables" "ip6tables")

remove_parent_jumps() {
    local cmd="$1"
    local parent="$2"
    local chain="$3"

    if ! "$cmd" -L "$parent" -n >/dev/null 2>&1; then
        return
    fi

    while IFS= read -r line; do
        [[ "$line" == "-A $parent "* ]] || continue
        [[ "$line" == *" -j $chain"* ]] || continue

        local rule_args="\${line#-A $parent }"
        # shellcheck disable=SC2086
        if "$cmd" -D "$parent" $rule_args 2>/dev/null; then
            echo "Removed jump rule from $parent -> $chain: $rule_args"
        fi
    done < <("$cmd" -S "$parent" 2>/dev/null || true)

    while "$cmd" -D "$parent" -j "$chain" 2>/dev/null; do
        echo "Removed legacy jump rule from $parent -> $chain"
    done
}

echo "Starting firewall cleanup for chains: \${CHAINS[*]}..."

for cmd in "\${TABLES[@]}"; do
    if ! command -v "\$cmd" &> /dev/null; then
        echo "\$cmd is not installed or not in PATH, skipping..."
        continue
    fi

    echo "--- Processing \$cmd ---"

    for chain in "\${CHAINS[@]}"; do
        for parent in "\${PARENTS[@]}"; do
            remove_parent_jumps "\$cmd" "\$parent" "\$chain"
        done

        if "\$cmd" -L "\$chain" -n >/dev/null 2>&1; then
            "\$cmd" -F "\$chain"
            echo "Flushed all rules inside \$chain"
        
            "\$cmd" -X "\$chain"
            echo "Deleted custom chain \$chain"
        else
            echo "Chain \$chain does not exist in \$cmd (already clean)."
        fi
    done

done

echo "Cleanup complete!"
`;

export function initCleanScript() {
  const scriptPath = join(dataPath, "clean.sh");

  try {
    writeFileSync(scriptPath, CLEAN_SCRIPT_CONTENT, { encoding: "utf-8" });
    chmodSync(scriptPath, 0o755);
    console.log(`[Init] Wrote clean.sh at ${scriptPath}.`);
  } catch (error) {
    console.error(`[Init] Failed to write clean.sh at ${scriptPath}:`, error);
  }
}
