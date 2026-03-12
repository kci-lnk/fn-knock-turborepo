import { existsSync, writeFileSync, chmodSync } from "node:fs";
import { join } from "node:path";
import { dataPath } from "./AppDirManager";

const CLEAN_SCRIPT_CONTENT = `#!/bin/bash

CHAIN_NAME="FN-KNOCK-FW"
PARENTS=("INPUT" "DOCKER-USER")
TABLES=("iptables" "ip6tables")

echo "Starting firewall cleanup for chain: \$CHAIN_NAME..."

for cmd in "\${TABLES[@]}"; do
    if ! command -v "\$cmd" &> /dev/null; then
        echo "\$cmd is not installed or not in PATH, skipping..."
        continue
    fi

    echo "--- Processing \$cmd ---"

    for parent in "\${PARENTS[@]}"; do
        while sudo \$cmd -D "\$parent" -j "\$CHAIN_NAME" 2>/dev/null; do
            echo "Removed jump rule from \$parent -> \$CHAIN_NAME"
        done
    done

    if sudo \$cmd -L "\$CHAIN_NAME" -n >/dev/null 2>&1; then
        sudo \$cmd -F "\$CHAIN_NAME"
        echo "Flushed all rules inside \$CHAIN_NAME"
        
        sudo \$cmd -X "\$CHAIN_NAME"
        echo "Deleted custom chain \$CHAIN_NAME"
    else
        echo "Chain \$CHAIN_NAME does not exist in \$cmd (already clean)."
    fi

done

echo "Cleanup complete!"
`;

export function initCleanScript() {
    const scriptPath = join(dataPath, 'clean.sh');

    if (!existsSync(scriptPath)) {
        try {
            writeFileSync(scriptPath, CLEAN_SCRIPT_CONTENT, { encoding: 'utf-8' });
            
            chmodSync(scriptPath, 0o755);
            
            console.log(`[Init] Created clean.sh at ${scriptPath} and granted execution permissions.`);
        } catch (error) {
            console.error(`[Init] Failed to create clean.sh at ${scriptPath}:`, error);
        }
    } else {
        console.log(`[Init] clean.sh already exists at ${scriptPath}.`);
    }
}