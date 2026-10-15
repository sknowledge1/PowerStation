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
| **file** | *in* | *s* |  |

#### LoadProfileJson

Load the same profile structure as JSON, without saving it. An all-automatic profile works during lifecycle restrictions and without telemetry. Clients own saving and reloading selections after service startup.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **json** | *in* | *s* |  |

#### EnumerateFans

Return the fan object paths for this platform. Unsupported systems return an empty list.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| \*\*\*\* | *out* | *ao* |  |

#### EnumerateSensors

Return stable, platform-selected temperature sensor objects. Profiles reference their Id properties.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| \*\*\*\* | *out* | *ao* |  |

#### GetStartupStatus

Read-only boot ID, current systemd invocation ID, and startup-restoration eligibility. Eligibility requires valid private first-invocation evidence, protected lifecycle, healthy telemetry, no fault and no selected profile. Invalid or unmanaged evidence returns empty identities and false. Clients must pin the unique bus owner and durably consume their boot attempt before loading a saved profile; this method grants no ownership or write privilege.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **boot\_id** | *out* | *s* |  |
| **invocation\_id** | *out* | *s* |  |
| **eligible** | *out* | *b* |  |

#### LoadStartupProfileJson

Load a saved JSON profile only on the identified first managed invocation while fans remain healthy, untouched and lifecycle-protected. Recheck identity and eligibility under the complete profile transaction lock; reject stale startup status without writing. Clients must still consume their own boot attempt before calling. Uses the same profile validation, access policy and rollback as LoadProfileJson.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **boot\_id** | *in* | *s* |  |
| **invocation\_id** | *in* | *s* |  |
| **json** | *in* | *s* |  |

#### LoadStartupProfile

Load a profile file on an eligible first managed invocation, using the same locked startup checks as LoadStartupProfileJson and the same approved directories and whole-file overrides as LoadProfile.

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **boot\_id** | *in* | *s* |  |
| **invocation\_id** | *in* | *s* |  |
| **file** | *in* | *s* |  |

### Signals
