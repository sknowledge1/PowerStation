# Fan profiles and platform configuration

Fan API **version 2** loads complete profiles through
`org.shadowblip.Fan` at `/org/shadowblip/Performance/Fan`.
`EnumerateFans` and `EnumerateSensors` return objects with stable `Id` values.
Use those IDs in profiles; object path indexes and hwmon numbering may change.

## Profiles

A profile has a name and one entry for every enumerated fan. Each entry selects
one mode:

| Mode | Behavior | Profile fields |
| --- | --- | --- |
| `automatic` | Let firmware control the fan. | `fan`, `mode` |
| `manual` | Request a fixed percentage, monitored by the service. | `fan`, `mode`, `percent` |
| `software_curve` | The service interpolates a curve using the hottest selected sensor. | `fan`, `mode`, `sensors`, `curve` |
| `firmware_curve` | Program the driver's native curve and let hardware interpolate it. | `fan`, `mode`, `sensors`, `curve` |

`AvailableModes` lists each fan's capabilities. Native curves require reviewed
hardware mode values, complete native point attributes, and a trusted binding to
the sensor used by that hardware. AYANEO 3 currently supports Automatic, Manual,
and software curves; it does not advertise native curves.

The packaged `quiet.yaml` lives under
`/usr/powerstation/platform/fan_curves`. An administrator can install a
same-filename YAML replacement under `/etc/powerstation/platform/fan_curves.d/`.
The replacement supplies the **whole profile**. Fields are never merged, and a
broken replacement is an error rather than a reason to load the packaged file.
`LoadProfile` accepts a YAML basename, or an absolute path directly inside either
of these directories. The distro replacement takes precedence in either case.
Paths outside these directories and links escaping their directory are rejected.

```yaml
name: Quiet
fans:
  - fan: ayaneo-3/system
    mode: software_curve
    sensors: [ayaneo-3/system/cpu]
    curve:
      - [45, 0]
      - [55, 15]
      - [65, 25]
      - [75, 35]
      - [85, 55]
      - [90, 75]
      - [95, 100]
```

Curve pairs contain degrees Celsius and a percentage from 0 to 100. Temperatures
must increase, percentages must not decrease, and the last point must reach
100% at or below 95°C. Platform minimums also apply. Native curves require the
exact driver point count, every percentage at or above the running floor, and
exactly the driver's bound sensor ID. Points are never resampled. Use a software
curve for fan-stop hysteresis or a different sensor selection.

```sh
busctl call org.shadowblip.PowerStation /org/shadowblip/Performance/Fan \
  org.shadowblip.Fan LoadProfile s quiet.yaml
```

`LoadProfileJson` accepts the identical structure as JSON. For example, Manual
40% and firmware Automatic on the AYANEO fan:

```json
{"name":"Manual 40%","fans":[{"fan":"ayaneo-3/system","mode":"manual","percent":40}]}
```

```json
{"name":"Automatic","fans":[{"fan":"ayaneo-3/system","mode":"automatic"}]}
```

Profiles are limited to 64 KiB. Unknown fields, unknown or duplicate IDs,
unsupported modes, invalid curves, and incomplete fan selections are errors.
The service validates the whole selection before changing hardware. If an
application error occurs after writes begin, it attempts verified Automatic on
**every affected fan** and reports the failure, including any failed handoff.

`CurrentProfileName` and `CurrentProfilePath` report the active selection. The
path is empty for JSON. Both are empty at startup and after a profile fault.
Fan `Mode` reports the current mode; during suspend it is `automatic` while the
live profile remains in memory. `Percent` is 0–100 and meaningful only when
`PercentValid` is true. Firmware Automatic and native curves have no public duty
readback. Sensor `TemperatureC` similarly requires `TelemetryValid`.

## Startup and lifecycle

Every new service process, clean restart, crash/watchdog restart, and reboot
starts in **firmware Automatic**. Applications own saving profiles and deciding
when to reload them. The daemon neither reads nor writes legacy
`/var/lib/powerstation/fan-state.yaml`; existing files are left untouched.

Successful suspend/resume retains only the live profile in memory. Before sleep,
the service blocks custom requests, including requests already queued on a fan
lock, and verifies a firmware handoff under logind's delay inhibitor. Resume
reacquires protection, discovers fresh paths and telemetry, and restores the live
selection. An Automatic profile remains callable during lifecycle restrictions
and without telemetry; selecting it during suspend cancels custom restoration.

The handoff retries within the reported `InhibitDelayMaxUSec` budget. Expiry,
logind owner loss/replacement, or unverified recovery blocks custom control and
makes the service unhealthy. The systemd watchdog and `ExecStopPost` helper
provide additional automatic recovery. A blocked kernel write cannot be
cancelled, and delay inhibitors expire or close on process death, so these
mechanisms cannot guarantee a completed handoff before every sleep.

## Trusted platform configuration

Reviewed `fan-*.yaml` files under `/usr/share/powerstation/platform` define exact
DMI alternatives, hwmon names, parent drivers, channels, stable fan/sensor IDs,
required safety sensors, driver mode values, PWM conversion/readback tolerance,
write order, running floor, hysteresis and stall/telemetry limits. See the
[packaged AYANEO entry](../rootfs/usr/share/powerstation/platform/fan-ayaneo-3.yaml).
This database is separate from editable profiles and the TDP TOML database.
Profiles cannot supply paths, mode register values, or critical thresholds.

DMI alternatives are ORed; every populated field within an alternative must
match exactly. Ambiguous device, fan, or sensor matches fail. Discovery uses
names and sensor labels rather than `hwmonN` indexes. Optional native points must
be complete and contiguous. `backend: software_curve` disables native curves;
`auto` advertises native curves only when attributes, a mode value and
`firmware_curve_sensor` binding exist; `hardware_curve` requires those facts.
The profile still explicitly selects software or firmware mode.

Required hardware safety sensors are monitored regardless of the profile's
chosen curve sensors. The provisional **95°C full-speed / 98°C firmware-handoff**
policy belongs to the service. Full speed also overrides a native curve if a
required safety sensor reaches 95°C; native control can resume after cooling.
Handoff transfers responsibility to firmware; it does not establish that firmware
can cool better than 100% software duty. These values await physical acceptance.

Unexpected driver mode changes, telemetry failures, stalls and readback failures
trigger Automatic. Mode readback can detect a writer switching modes; it cannot
detect another writer using the same mode. Existing fan daemons must be stopped
before enabling custom control. There is no sysfs exclusivity guarantee.

The recovery helper repeats trusted DMI/fan matching but requires only the mode
attribute, so failed temperature/RPM/PWM/curve discovery need not prevent
Automatic. Unsupported hardware returns empty enumerations.

The existing system-bus policy allows local callers to send requests to the
privileged service. Such callers may select profiles, but cannot choose arbitrary
hardware targets or lower the internal safety policy. Configuration directories
and service environment are administrator-controlled. Test overrides
`POWERSTATION_FAN_PLATFORM_DIR`, `POWERSTATION_FAN_SYSFS_ROOT`,
`POWERSTATION_FAN_PROFILE_DIR`, and `POWERSTATION_FAN_PROFILE_OVERRIDE_DIR` are
absent from the packaged unit. `POWERSTATION_FAN_STATE_PATH` is ignored.

See [Fan validation](fan-validation.md) for evidence and remaining release gates.
