# Fan DBus Interface API

## org.shadowblip.Fan

### Properties

| Name | Access | Type | Description |
| --- | :---: | :---: | --- |
| **ApiVersion** | *read* | *u* | Version 2 replaces the preset/custom-curve API with complete profiles. |
| **CurrentProfileName** | *read* | *s* | Active profile name, including during successful suspend. Empty after startup or a controller fault. |
| **CurrentProfilePath** | *read* | *s* | Resolved profile file path. Empty for JSON profiles, startup, or a controller fault. |

### Methods

#### LoadProfile

Load a complete YAML profile by filename in the approved profile directories. A same-name distro file replaces the packaged file. Validate every fan before writing; any application failure returns all affected fans to Automatic.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **file** | *in* | *s* | |

#### LoadProfileJson

Load the same profile structure as JSON, without saving it. An all-automatic profile works during lifecycle restrictions and without telemetry. Clients own saving and reloading selections after service startup.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **json** | *in* | *s* | |

#### EnumerateFans

Return the fan object paths for this platform. Unsupported systems return an empty list.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| \*\*\*\* | *out* | *ao* | |

#### EnumerateSensors

Return stable, platform-selected temperature sensor objects. Profiles reference their Id properties.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| \*\*\*\* | *out* | *ao* | |

### Signals
