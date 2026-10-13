# Fan.Device DBus Interface API

## org.freedesktop.DBus.Properties

### Methods

#### Get

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **interface_name** | *in* | *s* | |
| **property_name** | *in* | *s* | |
| \*\*\*\* | *out* | *v* | |

#### Set

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **interface_name** | *in* | *s* | |
| **property_name** | *in* | *s* | |
| **value** | *in* | *v* | |

#### GetAll

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **interface_name** | *in* | *s* | |
| \*\*\*\* | *out* | *a{sv}* | |

### Signals

#### PropertiesChanged

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **interface_name** | \*\* | *s* | |
| **changed_properties** | \*\* | *a{sv}* | |
| **invalidated_properties** | \*\* | *as* | |

## org.freedesktop.DBus.Introspectable

### Methods

#### Introspect

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| \*\*\*\* | *out* | *s* | |

### Signals

## org.freedesktop.DBus.Peer

### Methods

#### Ping

#### GetMachineId

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| \*\*\*\* | *out* | *s* | |

### Signals

## org.shadowblip.Fan.Device

### Properties

| Name | Access | Type | Description |
| --- | :---: | :---: | --- |
| **ActivePreset** | *read* | *s* | Selected preset name, or an empty string outside preset mode. |
| **ApiVersion** | *read* | *u* | Version of the fan D-Bus contract; currently 1. |
| **AutomaticVerified** | *read* | *b* | Whether the most recent ownership read or automatic-mode write verified firmware automatic mode. |
| **AvailableModes** | *read* | *as* | Supported selections, independent of temporary lifecycle restrictions. Includes preset only when Presets is nonempty. |
| **Backend** | *read* | *s* | Curve backend: software_curve or hardware_curve. Manual mode always uses the monitored software policy. |
| **Controlling** | *read* | *b* | Whether PowerStation currently owns manual or hardware-curve control. |
| **Curve** | *read* | *a(dy)* | Selected custom or preset curve as temperature/percentage pairs. A saved custom curve can remain visible in automatic or manual mode. |
| **CurvePointCount** | *read* | *q* | Required native point count for hardware curves; 0 for software curves. |
| **EffectivePercent** | *read* | *n* | Applied software duty or estimated hardware-curve duty, including driver quantization tolerance. Returns -1 when unavailable or firmware owns control. |
| **EmergencyHandoffTemperatureC** | *read* | *d* | Temperature at which custom control is abandoned for verified firmware automatic mode. |
| **Fault** | *read* | *s* | Last fan fault, or an empty string after successful configuration. Fresh telemetry alone does not acknowledge an earlier fault. |
| **FullSpeedTemperatureC** | *read* | *d* | Temperature at which software control forces 100 percent; every accepted curve reaches 100 percent by this threshold. |
| **Id** | *read* | *s* | Stable fan identifier within the selected platform. |
| **ManualPercent** | *read* | *y* | Saved manual target percentage, retained when another mode is selected. |
| **MaximumValidRpm** | *read* | *u* | Highest accepted fan RPM; greater readings invalidate telemetry. |
| **MaximumValidTemperatureC** | *read* | *d* | Highest accepted telemetry temperature in degrees Celsius. |
| **MinimumPercent** | *read* | *y* | Minimum accepted manual or curve percentage. |
| **MinimumRunningPercent** | *read* | *y* | Minimum software duty while the fan is running. |
| **MinimumValidTemperatureC** | *read* | *d* | Lowest accepted telemetry temperature in degrees Celsius. |
| **Mode** | *read* | *s* | Persisted selection: automatic, manual, curve, or preset. During suspend the selected mode is retained while firmware controls the fan. |
| **Name** | *read* | *s* | Human-readable fan name from the platform database. |
| **Presets** | *read* | *as* | Available preset names from the platform database. |
| **PwmMax** | *read* | *u* | Raw PWM value representing 100 percent. |
| **PwmMin** | *read* | *u* | Raw PWM value representing 0 percent. |
| **PwmReadbackTolerance** | *read* | *u* | Maximum permitted raw PWM readback difference caused by driver quantization. |
| **RestartTemperatureC** | *read* | *d* | Software fan restart threshold in degrees Celsius; restart occurs above this value. |
| **Rpm** | *read* | *u* | Last validated fan speed in RPM. Returns 0 when TelemetryValid is false. |
| **StallGraceSeconds** | *read* | *u* | Number of monitored one-second intervals allowed below StallRpm before automatic fallback. |
| **StallRpm** | *read* | *u* | RPM below which a running fan is considered stalled after StallGraceSeconds. |
| **StopTemperatureC** | *read* | *d* | Software fan-stop threshold in degrees Celsius when a zero-percent target is permitted. |
| **Suspended** | *read* | *b* | Whether the controller is paused for suspend or shutdown; custom selections are rejected while true. |
| **TelemetryValid** | *read* | *b* | Whether a complete trusted temperature/RPM sample is available. False after faults and during suspend until fresh telemetry is validated. |
| **TemperatureC** | *read* | *d* | Hottest configured sensor reading in degrees Celsius. Returns 0 when TelemetryValid is false. |

### Methods

#### SetAutomatic

Restore and verify firmware automatic mode, then persist that selection. Available during suspend and faults without temperature or RPM telemetry.

#### SetManual

Select and persist a manual percentage within the platform limits. The running floor and thermal overrides can raise the effective duty. Fails without changing the selection while lifecycle protection is unavailable.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **percent** | *in* | *y* | |

#### SetCurve

Select and persist increasing temperature/percentage points ending at 100 percent. Hardware curves require exactly CurvePointCount points; software curves accept two or more. Fails during lifecycle transitions.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **points** | *in* | *a(dy)* | |

#### ApplyPreset

Select and persist a named platform preset. Fails for unknown names or while lifecycle protection is unavailable.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **name** | *in* | *s* | |

### Signals
