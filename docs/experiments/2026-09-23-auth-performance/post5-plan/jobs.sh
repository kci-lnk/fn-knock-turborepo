# Generated immutable job list; sourced only after bundle hash verification.
run_cache() {
  run_case cache primary cache --routes session_hit --pairs 6 --warmup 20 --seconds 60 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 1 --renewals 64 --profile 0 --recovery-probe 0 --roles baseline,candidate
}
run_renewal() {
  run_case renewal primary renewal --routes grant_renewal --pairs 6 --warmup 20 --seconds 60 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 1 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 --roles baseline,candidate
}
run_grants() {
  run_case grants-1000 primary smoke --routes grant_hit --pairs 1 --warmup 5 --seconds 15 --concurrency 1 --clients 2 --accounts 1000 --sessions 1000 --grants 1000 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 --roles baseline,candidate
  run_case grants-10000 primary smoke --routes grant_hit --pairs 1 --warmup 5 --seconds 15 --concurrency 1 --clients 2 --accounts 1000 --sessions 1000 --grants 10000 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 --roles baseline,candidate
}
run_params() {
  run_case explore-reader-2 reader-2 smoke --routes bootstrap,session_hit,grant_hit,auto_ip_hit --pairs 1 --warmup 5 --seconds 15 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 --roles baseline,candidate
  run_case explore-reader-4 reader-4 smoke --routes bootstrap,session_hit,grant_hit,auto_ip_hit --pairs 1 --warmup 5 --seconds 15 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 --roles baseline,candidate
  run_case explore-compiler-s compiler-s smoke --routes bootstrap,session_hit,grant_hit,auto_ip_hit --pairs 1 --warmup 5 --seconds 15 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 --roles baseline,candidate
  run_case explore-compiler-2 compiler-2 smoke --routes bootstrap,session_hit,grant_hit,auto_ip_hit --pairs 1 --warmup 5 --seconds 15 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 --roles baseline,candidate
  run_case explore-compiler-3 compiler-3 smoke --routes bootstrap,session_hit,grant_hit,auto_ip_hit --pairs 1 --warmup 5 --seconds 15 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 --roles baseline,candidate
}
run_profile() {
  run_case profile-primary primary smoke --routes grant_hit,session_hit,auto_ip_hit --pairs 1 --warmup 20 --seconds 30 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 1 --recovery-probe 0 --roles baseline,candidate --profile-idle 5
  run_case profile-reader-2 reader-2 smoke --routes grant_hit,auto_ip_hit --pairs 1 --warmup 20 --seconds 30 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 1 --recovery-probe 0 --roles baseline,candidate --profile-idle 5
  run_case profile-reader-4 reader-4 smoke --routes grant_hit,auto_ip_hit --pairs 1 --warmup 20 --seconds 30 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 1 --recovery-probe 0 --roles baseline,candidate --profile-idle 5
}
run_soak() {
  run_case soak primary soak --routes auto_ip_hit --pairs 1 --warmup 20 --seconds 1800 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 --roles candidate
}
run_recovery() {
  run_case recovery recovery smoke --routes auto_ip_hit --pairs 1 --warmup 1 --seconds 3 --concurrency 16 --clients 2 --accounts 1000 --sessions 1000 --grants 100 --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 1 --roles baseline,candidate
}
