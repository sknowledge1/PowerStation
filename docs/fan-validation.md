# Fan validation

Fan API v2 implements [#29](https://github.com/ShadowBlip/PowerStation/issues/29)
with editable profiles and a separate trusted hardware database. Its software
validation gate permits updating PR #54. Physical acceptance and qualified
packages/images remain separate release gates.

## Reproducible software checks

The packaged startup safeguard and additive API v2 startup contract are documented in [fan-startup.md](fan-startup.md). Include its mandatory pre-start command and preserved private runtime directory in package inspection. Lifecycle checks additionally verify the actual first invocation, read-only startup status, locked startup-profile rejection after another selection, record preservation through clean/crash/watchdog restart and corrupt-record rejection before service exposure. These simulations do not qualify physical boot restoration on revised code.

Use the repository's Rust 1.88 Linux Dockerfile. It includes formatting/Clippy,
native and aarch64 libudev development files, and target-specific pkg-config
paths. Keep build output and Cargo home isolated; builds use the existing lock.
The only lockfile change for v2 adds the already-locked `serde_json` to the root
package's direct dependencies; no locked package versions change.

```sh
cargo fmt --all -- --check
cargo test --locked --all
cargo clippy --locked --all-targets -- -D warnings -A clippy::uninlined_format_args -A clippy::nonminimal_bool
cargo build --release --locked --target x86_64-unknown-linux-gnu
cargo build --release --locked --target aarch64-unknown-linux-gnu
make dist-archive TARGET_ARCH=x86_64-unknown-linux-gnu
make dist-archive TARGET_ARCH=aarch64-unknown-linux-gnu
cargo install cargo-generate-rpm --version 0.21.0 --locked
cargo generate-rpm --target x86_64-unknown-linux-gnu
cargo generate-rpm --target aarch64-unknown-linux-gnu
```

The two Clippy allowances concern existing upstream code. Inspect each archive
and RPM for the correct ELF/RPM architecture, exact payload bytes and paths,
0755 binary and 0644 configuration/unit/activation assets. Both formats must
contain the trusted AYANEO database and `/usr/powerstation/platform/fan_curves/quiet.yaml`.

**ARM RPM dependency qualification is pending.** Cross-generating an ARM RPM on
an x86 host does not reliably discover its direct runtime-library dependencies.
Correct architecture and payload checks are not dependency/install qualification.
Do not publish these candidates as qualified releases. Distro-native dependency,
installation and official image testing remain required.

Run the lifecycle harness on Linux with a working user systemd manager,
`dbus-daemon`, `busctl`, Python 3 and the native binary's runtime libraries:

```sh
make in-docker TARGET=test-fan-lifecycle-build
make test-fan-lifecycle
```

It uses fake DMI/sysfs, a private bus/logind fixture and a unique transient user
service with the packaged supervision settings. It never requests host suspend
or touches actual fan controls. It checks profile rejection, live profile
retention, queued-request protection, fresh resume discovery, sensor loss,
Automatic without telemetry, clean/crash/watchdog restarts in Automatic even
with valid or malformed legacy saved state, logind owner loss, bounded failed
handoff and malformed hardware configuration. It also compares all three custom
live D-Bus interfaces with checked-in XML while checking that the standard
interfaces remain available at runtime. The logind fixture rejects unrelated
`GetAll` while allowing the individual `InhibitDelayMaxUSec` read.

To regenerate fan bindings without accessing real hardware:

```sh
python3 tests/fan_lifecycle.py path/to/powerstation path/to/fan_test_logind --write-bindings
```

Fan Markdown is generated from these bindings with the
`org.shadowblip.Fan` interface filter in `make docs`. Standard D-Bus boilerplate
is omitted from fan documentation.

Unit tests additionally cover YAML/JSON equivalence, whole-file overrides,
invalid/oversized input, sensor selection independent of required safety sensors,
native point counts/sensor binding/running floor, conversion/readback tolerances,
stalls, native full-speed override, complete-profile validation and multi-fan
rollback. These simulations do not validate electrical behavior, firmware timing,
real kernel suspend, or thermal response. Exact tested commit, test counts and
artifact hashes are recorded in the PR validation report.

## AYANEO evidence and remaining gates

The pinned Linux [`ayaneo-ec` source at 2643187](https://github.com/torvalds/linux/blob/2643187ccb8628144246ee9d44da5e3ac428f9c3/drivers/platform/x86/ayaneo-ec.c)
provides the board match, hwmon/driver names and enable values. PWM 0–255 maps to
integer EC percentages, explaining the three-unit raw readback tolerance kept
inside the trusted controller. The service also uses a reviewed write order.
Future device records must capture kernel, DMI, driver, sensor labels and actual
readbacks rather than infer physical support from schema matching.

Historical results at `e6baca4e9ef4a04df2884b8803a6ca6deb167ec0` cover 8840U
Quiet/Manual and automatic recovery. Manual 20% read `pwm1=51`, 1502 RPM; one
Quiet sample at 49.625°C read `pwm1=25`, 1061 RPM. Those are individual samples.

The later **255e6805347d605a13106d711c5e565495985633** device record, reconciled
with `Ayaneo3-Part2/STATUS.md`, used AYANEO 3 8840U / BIOS 1.00 on official
Bazzite 44.20261012.1, kernel 7.2.9-ogc3.1.fc44.x86_64. It includes real
s2idle suspend/resume, reboot, clean-stop D-Bus activation, SIGKILL/watchdog and
sensor-fault recovery through a temporary service binary/platform namespace.

An **uninterrupted five-minute CPU-only Quiet test at unchanged 9 W** passed
with **352 samples and a 64.000°C peak**. All 16 stress-ng workers completed;
cooldown ended in verified firmware Automatic. A scoped inhibitor and sleep-gap
guard protected continuity, and power/thermal settings matched before and after.
The earlier interrupted attempt remains separate evidence.

These results belong to **255e680**, whose daemon persisted selections and
exposed API v1. They are **not device validation of API v2 or its startup policy**.
The later API v2 build **1b7330eadb17620cec1870539817fcb928942c4c** has separate
private 8840U evidence: attended fan modes, real normal-boot restoration,
crash-before-first-client refusal, native restart/SIGKILL and sensor-fault
recovery, suspend/resume and rollback. Its uninterrupted five-minute, 16-thread
CPU-only Quiet observation at unchanged 9 W records **342 samples and a 65.5°C
peak**, followed by native Automatic cleanup. Those API v2 results do not
replace or relabel the 255e680 record. They use private native binaries and
overrides, not qualified official package delivery.

Decky's separate API v2 migration and private acceptance are complete.
OpenGamepadUI's separate client revision uses the additive startup-status and
guarded startup-load methods. No physical test of these new methods or the
revised native/OpenGamepadUI heads is claimed. Godot/native integration against
a private bus and fake sysfs is software evidence only.

| Release gate | Status |
| --- | --- |
| Revised-code device startup/restart/suspend/fault behavior | Pending new device testing |
| Full operating-range response, running floor and fan-stop hysteresis | Pending physical acceptance |
| Provisional 95°C full speed / 98°C firmware handoff | Pending thermal policy acceptance; handoff transfers responsibility, not proof of stronger cooling |
| HX 370 AYANEO variant | Compatibility assumed from shared interfaces and 8840U results; not separately physically tested |
| Native hardware curves | Simulated coverage only; no packaged platform advertises them |
| ARM RPM dependency/install qualification | Pending distro-native qualification |
| Official testing/stable packages and clean official-image installation | Pending; private namespace testing does not satisfy this gate |

[Logind delay inhibitors](https://systemd.io/INHIBITOR_LOCKS/) are bounded and
close on process death. Firmware writes can fail or block, so watchdog recovery
cannot guarantee a completed handoff before every sleep.
