# First managed fan-service invocation

Fan API v2 always starts Automatic and does not read legacy saved profiles. Clients may save preferences, but must not replay custom control after a fault or service restart.

The packaged systemd unit runs `powerstation --record-fan-startup` in mandatory ExecStartPre before the daemon. RuntimeDirectory `powerstation-fan-startup` has mode 0700 and RuntimeDirectoryPreserve=yes, preserving evidence through stop/start, crash and watchdog recovery until reboot. The helper writes no hardware and does not restore a profile.

`/run/powerstation-fan-startup/first.json` is a private, single-link regular file with mode 0600 owned by the service user (root in the packaged system unit). Its version-1 JSON contains exactly `version`, `boot_id` (kernel boot UUID) and `invocation_id` (32 lowercase hexadecimal characters from systemd INVOCATION_ID). The first creation is exclusive. Later invocations validate but never replace that first identity.

Missing directories, symlinks, insecure ownership/modes, stale boot identity, malformed/partial/oversized records and unsupported versions fail the mandatory pre-start helper. The file is preserved. The daemon cannot accept requests from an unrecorded managed invocation; existing ExecStopPost Automatic cleanup remains active. A partial first record intentionally blocks later startup rather than misidentifying a restart as a normal boot. Reboot clears this runtime-only evidence; clients must not delete or repair it automatically.

Before opening the first record for creation, the helper exclusively creates and syncs root-owned 0700 `first.json.attempt`. An existing attempt requires a valid existing record; a missing record is a failed attempt and is never created later. This preserves the failure fence even when record-file creation itself fails. Existing valid first records remain unchanged during an in-boot package upgrade.

A restoring client must validate the root-owned record, compare both boot and current systemd InvocationID, bind the service's unique bus owner Unix PID to MainPID, and confirm fresh healthy telemetry, no fault and no active profile. Consume the client-side boot attempt durably before sending; uncertainty, a changed owner or invalid evidence forbids automatic replay. This record is evidence. Profile validation and native Automatic recovery remain authoritative.

The helper's `POWERSTATION_FAN_STARTUP_PATH` environment override exists for trusted service configuration and isolated tests, like the existing fake platform/sysfs paths. The private lifecycle harness uses its unique transient unit's runtime directory and actual invocation IDs. A directly executed daemon is not proof of managed first startup and cannot qualify guarded boot restoration.

Tests cover first/later invocations, private permissions, corrupt/interrupted records, wrong identity, symlinks, hard links, nonregular files and bounded reads. The real systemd lifecycle fixture verifies preservation across clean/crash/watchdog restarts and failure before D-Bus exposure with corrupt evidence. No fixture result substitutes for physical fan acceptance.

## Unprivileged client contract

API v2 adds `GetStartupStatus() -> (boot_id, invocation_id, eligible)`. It exposes
only the current boot/invocation identity and eligibility, leaving both recorder
paths private. Missing, corrupt, untrusted or unmanaged evidence produces empty
identities and false. A later managed invocation reports its current identity
but is never eligible. Status reads never create or repair evidence and write
no hardware. Eligibility additionally requires protected lifecycle, healthy
telemetry, verified Automatic on every fan, no fault and no selected profile.

A read alone cannot close the race between checking eligibility and writing a
profile. `LoadStartupProfile(boot_id, invocation_id, file)` and
`LoadStartupProfileJson(boot_id, invocation_id, json)` therefore recheck those
identities and eligibility while holding the existing whole-profile transaction
and controller locks. Stale status, another client's selection (including an
explicit Automatic profile), faults, suspend restrictions and later invocations
are rejected without writing. These methods use the same approved directories,
profile validation, rollback and D-Bus caller policy as ordinary loads; they add
no ownership claim or privilege. API version remains 2: the unmerged v2 proposal
gains these additive methods, and clients requiring guarded restoration must
require them rather than emulate them with an ordinary load.

An unprivileged client pins the unique owner for status/loads, compares the
reported boot with the kernel boot UUID and durably consumes its own per-user
boot attempt before reading preferences or calling a startup load. It must
never automatically retry uncertainty or consume a fresh attempt on service
recovery. The native locked guard is authoritative across clients; preferences
and per-user fences are not privileged ownership. A successful startup load
also prevents another client from automatically replacing that profile.

Existing privileged clients may keep validating the private record directly.
The new contract does not change record permissions, service supervision,
fresh-process Automatic behavior or the native live suspend/resume handoff.
Simulated coverage of the additive methods is separate from all physical
results recorded against earlier heads.
