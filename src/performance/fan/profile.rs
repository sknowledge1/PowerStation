//! Untrusted selections. Hardware addressing and safety policy live elsewhere.
use super::config::{ConfigError, CurvePoint};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_PROFILE_DIR: &str = "/usr/powerstation/platform/fan_curves";
pub const DEFAULT_OVERRIDE_DIR: &str = "/etc/powerstation/platform/fan_curves.d";
const MAX_PROFILE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FanMode {
    #[default]
    Automatic,
    Manual,
    FirmwareCurve,
    SoftwareCurve,
}

impl FanMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
            Self::FirmwareCurve => "firmware_curve",
            Self::SoftwareCurve => "software_curve",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanProfile {
    pub fan: String,
    pub mode: FanMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curve: Vec<(f64, u8)>,
}

impl FanProfile {
    pub fn automatic(fan: String) -> Self {
        Self {
            fan,
            mode: FanMode::Automatic,
            percent: None,
            sensors: Vec::new(),
            curve: Vec::new(),
        }
    }

    pub fn points(&self) -> Vec<CurvePoint> {
        self.curve
            .iter()
            .map(|&(temperature_c, percent)| CurvePoint {
                temperature_c,
                percent,
            })
            .collect()
    }

    pub(crate) fn validate_shape(&self) -> Result<(), ConfigError> {
        let valid = match self.mode {
            FanMode::Automatic => {
                self.percent.is_none() && self.sensors.is_empty() && self.curve.is_empty()
            }
            FanMode::Manual => {
                self.percent.is_some_and(|percent| percent <= 100)
                    && self.sensors.is_empty()
                    && self.curve.is_empty()
            }
            FanMode::FirmwareCurve | FanMode::SoftwareCurve => {
                self.percent.is_none() && !self.sensors.is_empty() && self.curve.len() >= 2
            }
        };
        let mut sensors = HashSet::new();
        if self.fan.is_empty()
            || !valid
            || self
                .sensors
                .iter()
                .any(|id| id.is_empty() || !sensors.insert(id))
        {
            return Err(ConfigError(format!(
                "invalid fields for fan {} mode {}",
                self.fan,
                self.mode.name()
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub fans: Vec<FanProfile>,
}

impl Profile {
    pub fn from_json(raw: &str) -> Result<Self, ConfigError> {
        check_size(raw.len())?;
        let profile: Self =
            serde_json::from_str(raw).map_err(|error| ConfigError(error.to_string()))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn from_yaml(raw: &str) -> Result<Self, ConfigError> {
        check_size(raw.len())?;
        let profile: Self = serde_yaml::from_str(raw)?;
        profile.validate()?;
        Ok(profile)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let mut fans = HashSet::new();
        if self.name.trim().is_empty() || self.name.len() > 128 || self.fans.is_empty() {
            return Err(ConfigError(
                "profile requires a name and at least one fan".into(),
            ));
        }
        for fan in &self.fans {
            fan.validate_shape()?;
            if !fans.insert(&fan.fan) {
                return Err(ConfigError(format!("duplicate fan ID {}", fan.fan)));
            }
        }
        Ok(())
    }
}

fn check_size(size: usize) -> Result<(), ConfigError> {
    if size > MAX_PROFILE_BYTES {
        return Err(ConfigError("profile exceeds 64 KiB".into()));
    }
    Ok(())
}

/// Accept a YAML basename, or an absolute path directly inside either approved
/// directory. Overrides replace the entire file. Never fall back from a broken
/// override to a packaged profile, and never follow a link outside its directory.
pub fn load_profile(
    file: &str,
    packaged: &Path,
    overrides: &Path,
) -> Result<(Profile, PathBuf), ConfigError> {
    let requested = Path::new(file);
    let name = requested
        .file_name()
        .ok_or_else(|| ConfigError("profile filename required".into()))?;
    let simple = requested.components().count() == 1
        && matches!(requested.components().next(), Some(Component::Normal(_)));
    if !simple
        && (!requested.is_absolute()
            || ![packaged, overrides].contains(&requested.parent().unwrap_or(Path::new(""))))
    {
        return Err(ConfigError(
            "profile must be a filename in an approved profile directory".into(),
        ));
    }
    if !matches!(
        requested.extension().and_then(|value| value.to_str()),
        Some("yaml" | "yml")
    ) {
        return Err(ConfigError(
            "profile filename must end in .yaml or .yml".into(),
        ));
    }
    let replacement = overrides.join(name);
    let path = match fs::symlink_metadata(&replacement) {
        Ok(_) => replacement,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => packaged.join(name),
        Err(error) => return Err(error.into()),
    };
    let directory = path.parent().unwrap().canonicalize()?;
    let canonical = path.canonicalize()?;
    if canonical.parent() != Some(directory.as_path()) || !canonical.is_file() {
        return Err(ConfigError(
            "profile must be a regular file inside its approved directory".into(),
        ));
    }
    // Bound reads before parsing even if an administrator accidentally installs
    // an oversized file. The roots themselves remain administrator-controlled.
    use std::io::Read;
    let mut raw = String::new();
    fs::File::open(&canonical)?
        .take((MAX_PROFILE_BYTES + 1) as u64)
        .read_to_string(&mut raw)?;
    Ok((Profile::from_yaml(&raw)?, canonical))
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "name: Quiet\nfans:\n  - fan: ayaneo-3/system\n    mode: software_curve\n    sensors: [ayaneo-3/system/cpu]\n    curve: [[45, 0], [95, 100]]\n";

    #[test]
    fn yaml_json_equivalence_and_closed_fields() {
        let profile = Profile::from_yaml(YAML).unwrap();
        assert_eq!(
            Profile::from_json(&serde_json::to_string(&profile).unwrap()).unwrap(),
            profile
        );
        assert!(Profile::from_yaml(&format!("{YAML}sysfs: /etc/shadow\n")).is_err());
        assert!(Profile::from_json(&" ".repeat(MAX_PROFILE_BYTES + 1)).is_err());
    }

    #[test]
    fn rejects_wrong_mode_fields_duplicates_and_bad_percentages() {
        for raw in [
            r#"{"name":"x","fans":[{"fan":"f","mode":"manual","percent":101}]}"#,
            r#"{"name":"x","fans":[{"fan":"f","mode":"automatic","percent":10}]}"#,
            r#"{"name":"x","fans":[{"fan":"f","mode":"manual"}]}"#,
            r#"{"name":"x","fans":[{"fan":"f","mode":"automatic"},{"fan":"f","mode":"automatic"}]}"#,
            r#"{"name":"x","fans":[{"fan":"f","mode":"software_curve","sensors":["s","s"],"curve":[[45,10],[95,100]]}]}"#,
            r#"{"name":"x","fans":[{"fan":"f","mode":"preset"}]}"#,
        ] {
            assert!(Profile::from_json(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn override_replaces_whole_profile_and_invalid_override_fails() {
        let root =
            std::env::temp_dir().join(format!("powerstation-profiles-{}", std::process::id()));
        let packaged = root.join("usr");
        let overrides = root.join("etc");
        fs::create_dir_all(&packaged).unwrap();
        fs::create_dir_all(&overrides).unwrap();
        fs::write(packaged.join("quiet.yaml"), YAML).unwrap();
        let (original, _) = load_profile("quiet.yaml", &packaged, &overrides).unwrap();
        assert_eq!(original.name, "Quiet");
        fs::write(
            overrides.join("quiet.yaml"),
            "name: Distro\nfans: [{fan: ayaneo-3/system, mode: automatic}]\n",
        )
        .unwrap();
        let (replacement, path) = load_profile(
            packaged.join("quiet.yaml").to_str().unwrap(),
            &packaged,
            &overrides,
        )
        .unwrap();
        assert_eq!(replacement.name, "Distro");
        assert_eq!(replacement.fans[0].mode, FanMode::Automatic);
        assert_eq!(path, overrides.join("quiet.yaml"));
        fs::write(overrides.join("quiet.yaml"), "name: incomplete\n").unwrap();
        assert!(load_profile("quiet.yaml", &packaged, &overrides).is_err());
        assert!(load_profile("../quiet.yaml", &packaged, &overrides).is_err());
        assert!(load_profile("/etc/shadow", &packaged, &overrides).is_err());
        fs::remove_file(overrides.join("quiet.yaml")).unwrap();
        std::os::unix::fs::symlink(packaged.join("quiet.yaml"), overrides.join("quiet.yaml"))
            .unwrap();
        assert!(load_profile("quiet.yaml", &packaged, &overrides).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn packaged_quiet_curve_is_preserved() {
        let profile = Profile::from_yaml(include_str!(
            "../../../rootfs/usr/powerstation/platform/fan_curves/quiet.yaml"
        ))
        .unwrap();
        assert_eq!(profile.name, "Quiet");
        assert_eq!(
            profile.fans[0].curve,
            vec![
                (45.0, 0),
                (55.0, 15),
                (65.0, 25),
                (75.0, 35),
                (85.0, 55),
                (90.0, 75),
                (95.0, 100)
            ]
        );
    }
}
