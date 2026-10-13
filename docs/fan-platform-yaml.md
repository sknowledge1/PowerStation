# Fan platform YAML

Fan support is enabled only by reviewed files named `fan-*.yaml` under
`/usr/share/powerstation/platform`. The fan schema is independent of the
existing TDP TOML database.

Each file declares `schema_version: 1` and one or more devices. DMI alternatives
are ORed; every populated field inside one alternative must match exactly. More
than one matching device is an error. A fan entry then identifies one hwmon by
its `name`, resolved parent driver, and PWM channel. Temperature sources use an
hwmon name and sensor label, so changing `hwmonN` indexes do not affect discovery.

```yaml
schema_version: 1
devices:
  - id: example-handheld
    name: Example Handheld
    dmi:
      - sys_vendor: Example
        product_name: Example Handheld
      - board_vendor: Example
        board_name: Example Board
    fans:
      - id: system
        name: System Fan
        hwmon:
          name: example_ec
          parent_driver: example-ec
          channel: 1
        temperature_sources:
          - hwmon_name: k10temp
            label: Tctl
            required: true
        backend: auto
        mode_values:
          automatic: 2
          manual: 1
          hardware_curve: null
        manual_write_order: enable_then_pwm
        limits:
          pwm_min: 0
          pwm_max: 255
          pwm_readback_tolerance: 0
          minimum_percent: 0
          minimum_running_percent: 20
          stop_temperature_c: 40.0
          restart_temperature_c: 45.0
          stall_rpm: 300
          stall_grace_seconds: 5
          full_speed_temperature_c: 90.0
          emergency_handoff_temperature_c: 95.0
          minimum_valid_temperature_c: 5.0
          maximum_valid_temperature_c: 105.0
          maximum_valid_rpm: 10000
        presets: {}
```

`backend` accepts `auto`, `software_curve`, or `hardware_curve`. Auto discovery
uses a software curve when only one PWM control is present. A complete,
contiguous set of `pwmN_auto_pointX_temp` and `pwmN_auto_pointX_pwm` attributes
can use the hardware backend only when `hardware_curve` supplies the documented
driver mode value, `minimum_percent` is at least `minimum_running_percent`, and
every preset contains exactly the native point count. Hardware curves cannot
implement the software fan-stop hysteresis or its discontinuous running floor.
Auto uses software control for those policies or for missing hardware mode
values. An explicit `hardware_curve` request rejects unsupported policy.
Partial point sets are rejected. Custom hardware curves must also contain
exactly the native point count; points are never silently resampled.

Mode values, PWM range and quantization tolerance, write order, temperature
limits, hysteresis, RPM limits, and presets are hardware facts or policies that
must be justified during review. Adding a YAML file must include fake-sysfs tests;
physical support should be claimed only for models that were exercised on real
hardware.

The initial AYANEO 3 entry deliberately has no CPU-model restriction because its
DMI and `ayaneo_ec` interface are shared across the product family. Its 95 C
full-speed and 98 C firmware-handoff values remain proposed policy for maintainer
review. Hardware validation currently covers the Ryzen 7 8840U model; the Ryzen
AI 9 HX 370 model is configuration-schema coverage until tested on that device.
Driver evidence, the basis and limits of the proposed policy, and reproducible
validation commands are recorded in [Fan validation](fan-validation.md).

On a discovery, telemetry, ownership, readback, stall, lifecycle, or watchdog
fault, PowerStation restores the configured automatic mode. Before suspend it
uses logind's delay inhibitor to attempt a verified handoff, then rediscovers
paths and restores saved control after resume. Custom configuration calls are
blocked before the handoff starts and remain blocked until lifecycle protection
is ready again. The selected mode is retained during a successful suspend,
while `Controlling` and `TelemetryValid` are false.

Failed handoffs retry within the reported `InhibitDelayMaxUSec` budget, with
blocking sysfs work kept off the asynchronous runtime. Expiry or failure makes
the service unhealthy and prevents further custom control. A timeout cannot
cancel a blocked kernel operation; a late worker can only restore automatic
mode. The watchdog and `ExecStopPost` provide additional best-effort recovery.
A delay inhibitor expires and is released when its process dies, so neither
holding it nor the post-stop helper guarantees completion before sleep. Loss
or replacement of logind's D-Bus owner also makes the service unhealthy and
forces automatic fallback.

The recovery command repeats the exact DMI match and fan hwmon name, parent
driver, and channel match, but intentionally does not require temperature, RPM,
PWM telemetry, or auto-point discovery. This limited path lets shutdown recovery
restore the configured automatic enable value after the telemetry failure that
caused the service to exit, without widening the set of writable devices.

Isolated lifecycle tests can redirect every fan-controlled path with
`POWERSTATION_FAN_PLATFORM_DIR`, `POWERSTATION_FAN_SYSFS_ROOT`,
and `POWERSTATION_FAN_STATE_PATH`. These
variables are intentionally absent from the packaged systemd unit; normal service
runs use the packaged platform database, real sysfs, and persistent system state.

Only PowerStation's own saved selections are restored. Import of third-party
AYANEO daemon state is deferred; first use remains in firmware Automatic.
An existing fan daemon must be stopped before enabling custom control because
two writers using the same manual mode cannot be distinguished by mode readback.
