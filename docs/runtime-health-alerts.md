# Runtime health alert lifecycle

The monitor probes every 5 seconds with a 2-second timeout. An incident starts after
three consecutive failures and recovers after two consecutive successes.

## Initial readiness

Gateway process, dataplane, auth bridge and config sync receive at most 60 seconds
from management process initialization to become ready. Failed samples during this
window retain their actual reason and report `degraded`; they do not claim readiness
or increment the failure count. The first successful sample ends the grace period
for that component immediately. Storage and management do not receive this grace.
After the deadline, normal failure counting applies even if startup never succeeds.
Existing incidents are never muted by the startup grace.

## Dependency failures and recovery

When the gateway process is unhealthy, dataplane and auth bridge become `blocked`.
This suppresses new downstream incidents but preserves an incident already emitted
for either component. After the dependency recovers, two successful probes close
that same incident and emit its original ID and full duration. A component blocked
without an incident does not emit a synthetic recovery event.

A single successful sample followed by further failures does not create a second
incident. The original incident remains open until two consecutive successes.

## Planned stops

The existing instance-bound FPK handshake remains authoritative; see
[runtime-planned-stop-design.md](runtime-planned-stop-design.md). Startup grace
cannot replace stop intent, renew its lease or mute storage failures. Probe
results spanning a stop/cancel generation change are discarded by that protocol.

## Evidence and verification

The September 12, 2026 investigation found 13 retained health-failure events in the
full installation: five gateway process, four dataplane and four auth bridge.
Four process alerts coincided with historical planned stops. The latest startup
sequence raised alerts at 14:47:06–12 CST, then returned to healthy by 14:47:18.
The blocked-state implementation discarded downstream incident IDs, explaining
missing matching recovery events despite healthy live status. Historical events
are retained unchanged; this fix does not invent recovery times for old incidents.

Regression tests cover bounded startup, first-success cutoff, genuine storage and
startup-timeout alerts, blocked recovery correlation, flapping and no-incident
recovery. Existing planned-stop tests cover ACK, stale samples, cancellation,
expiry, instance replacement and shell stop ordering.
