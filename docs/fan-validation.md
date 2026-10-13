# Fan validation

The fan subsystem implements issue [#29](https://github.com/ShadowBlip/PowerStation/issues/29):
software curves for PWM-only devices and reviewed hardware auto-point curves.
The AYANEO entry is an initial platform proposal. Importing another daemon's
saved state is deferred so this change owns only PowerStation configuration.

## Reproducible checks

Use the repository's Rust 1.88 Linux Docker environment for formatting, locked
tests, release builds, and distribution checks. Install the Rust `rustfmt` and
`clippy` components in that environment for the corresponding checks.

```sh
cargo fmt --all -- --check
cargo test --locked --all
cargo clippy --locked --all-targets -- -D warnings -A clippy::uninlined_format_args -A clippy::nonminimal_bool
cargo build --release --locked --target x86_64-unknown-linux-gnu
cargo build --release --locked --target aarch64-unknown-linux-gnu
make dist TARGET_ARCH=x86_64-unknown-linux-gnu
make dist TARGET_ARCH=aarch64-unknown-linux-gnu
```

The two Clippy allowances concern existing upstream code, not fan code.
Do not add new occurrences under those allowances. Check that both archives and
RPMs include the fan YAML and that build commands leave Cargo.lock unchanged.

Run the checked-in lifecycle harness on Linux with a working user systemd
manager, `dbus-daemon`, `busctl`, Python 3, and the native binary's runtime
libraries installed:

```sh
make in-docker TARGET=test-fan-lifecycle-build
make test-fan-lifecycle
```

The harness creates a private D-Bus daemon, a fake logind fixture, a unique
transient user service, and temporary fan/DMI/temperature/state files. It copies
the packaged service properties, redirecting the executable and fan paths.
It checks custom-call rejection during suspend, path renumbering, sensor loss,
clean stop/restart, SIGKILL, SIGSTOP/watchdog recovery, logind owner loss, bounded
failed handoff, and malformed YAML. The fixture models a 250 ms inhibitor
deadline; it never requests host suspend or writes real fan controls. Cleanup
stops only its transient unit and fixture processes. GitHub Actions runs this
harness in a separate Linux job.

These checks validate software behavior against simulated files and signals.
They do not reproduce electrical faults, firmware timing, thermal response,
real kernel suspend, or the physical behavior of a hardware fan curve.

## AYANEO driver evidence

The Linux [`ayaneo-ec` driver at 2643187](https://github.com/torvalds/linux/blob/2643187ccb8628144246ee9d44da5e3ac428f9c3/drivers/platform/x86/ayaneo-ec.c)
matches board vendor `AYANEO` and board name `AYANEO 3`, registers hwmon
`ayaneo_ec`, and exposes one PWM/fan channel. Its platform driver is `ayaneo-ec`.
Sysfs enable value 1 selects manual mode and 2 selects firmware automatic mode.
PWM writes accept 0–255, convert to integer EC percent, and read back on the
0–255 scale. That round trip can differ by up to 3 raw PWM units, which explains
the configured tolerance. The driver's hibernation freeze callback also hands
manual control back to automatic. PowerStation attempts and verifies its own
handoff for logind sleep requests before releasing the delay inhibitor.

This source inspection is separate from device validation. The original
physical report did not retain a kernel version or complete raw DMI/driver dump.
Record `uname -r`, `lscpu`, `udevadm info -a /sys/class/dmi/id`, the driver link,
hwmon name, sensor labels, and mode/PWM readbacks with each future hardware test.

## Proposed policy and physical coverage

| Setting | Basis and remaining evidence |
| --- | --- |
| 0% while cool; 42 C stop and 45 C restart | Proposed software policy with a 3 C hysteresis band; covered by fake-sysfs regressions. |
| 10% running floor | The historical 8840U Quiet sample showed `pwm1=25` and 1061 RPM at 49.625 C. This establishes one operating sample, not all thermal/load conditions. |
| 95 C full speed; 98 C firmware handoff | Proposed control thresholds: full speed precedes handoff by 3 C. They are not vendor-certified thermal limits and have not been validated by a sustained thermal test. |
| Seven-point Quiet preset | Proposed interpolation policy; the sampled Quiet behavior does not validate the complete curve. |

Historical physical validation used the 8840U AYANEO 3 at
`e6baca4e9ef4a04df2884b8803a6ca6deb167ec0`: Quiet/manual control, SIGKILL/watchdog
recovery, and clean-stop automatic restoration were reported passing. Manual 20%
read back as `pwm1=51` at 1502 RPM. Those results belong to that build.
Current automated checks do not constitute a new physical test.

The YAML matches product or board identity across CPU variants. The HX 370
variant remains untested hardware; schema/DMI coverage is not physical support
validation. No packaged platform currently validates native hardware curves.
Future entries must establish their driver's sensor mapping and mode semantics
as well as native point counts, running floor, and thermal behavior.

Logind delay inhibitors are bounded and released on process death, as described
in [systemd's inhibitor documentation](https://systemd.io/INHIBITOR_LOCKS/).
Automatic recovery can fail when kernel/firmware writes fail; watchdog cleanup
does not make that handoff an unconditional prerequisite for suspend.
