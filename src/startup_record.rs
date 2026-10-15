//! First systemd invocation evidence. This never reads or writes fan hardware.
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const RECORD_PATH: &str = "/run/powerstation-fan-startup/first.json";
const MAX_RECORD_BYTES: u64 = 4096;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    boot_id: String,
    invocation_id: String,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn valid_boot(value: &str) -> bool {
    value.len() == 36
        && [8, 13, 18, 23]
            .iter()
            .all(|index| value.as_bytes()[*index] == b'-')
        && valid_hex(&value.replace('-', ""), 32)
}

fn checked_record(file: &mut File, boot_id: &str, uid: u32) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() > MAX_RECORD_BYTES
    {
        return Err(invalid("untrusted fan startup record"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(invalid("oversized fan startup record"));
    }
    let record: Record =
        serde_json::from_slice(&bytes).map_err(|_| invalid("corrupt fan startup record"))?;
    if record.version != 1 || record.boot_id != boot_id || !valid_hex(&record.invocation_id, 32) {
        return Err(invalid("fan startup record is not valid for this boot"));
    }
    Ok(())
}

fn record_first(path: &Path, boot_id: &str, invocation_id: &str, uid: u32) -> io::Result<()> {
    if !valid_boot(boot_id) || !valid_hex(invocation_id, 32) {
        return Err(invalid("invalid boot or systemd invocation identity"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| invalid("missing startup record directory"))?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent)?;
    let metadata = directory.metadata()?;
    if metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 {
        return Err(invalid("untrusted fan startup directory"));
    }
    let name = CString::new(
        path.file_name()
            .ok_or_else(|| invalid("missing record name"))?
            .as_encoded_bytes(),
    )
    .map_err(|_| invalid("invalid record name"))?;
    // Exclusive creation is the durable first-attempt fence. A partial record is
    // deliberately retained: replacing it could make a later invocation look first.
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if descriptor < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::AlreadyExists {
            return Err(error);
        }
        let descriptor = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut existing = unsafe { File::from_raw_fd(descriptor) };
        return checked_record(&mut existing, boot_id, uid);
    }
    let mut file = unsafe { File::from_raw_fd(descriptor) };
    let record = Record {
        version: 1,
        boot_id: boot_id.into(),
        invocation_id: invocation_id.into(),
    };
    file.write_all(&serde_json::to_vec(&record)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    directory.sync_all()
}

pub fn record_service_start() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::var_os("POWERSTATION_FAN_STARTUP_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(RECORD_PATH));
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    let invocation_id = std::env::var("INVOCATION_ID")?;
    record_first(&path, boot_id.trim(), &invocation_id, unsafe {
        libc::geteuid()
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, DirBuilderExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};
    const BOOT: &str = "12345678-1234-1234-1234-123456789abc";
    const FIRST: &str = "11111111111111111111111111111111";
    const SECOND: &str = "22222222222222222222222222222222";
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "powerstation-startup-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .unwrap();
            Self(path)
        }
        fn record(&self, invocation: &str) -> io::Result<()> {
            record_first(&self.0.join("first.json"), BOOT, invocation, unsafe {
                libc::geteuid()
            })
        }
        fn path(&self) -> PathBuf {
            self.0.join("first.json")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn first_identity_is_preserved_across_all_later_invocations() {
        let fixture = Fixture::new();
        fixture.record(FIRST).unwrap();
        let original = std::fs::read(fixture.path()).unwrap();
        fixture.record(SECOND).unwrap();
        fixture.record(FIRST).unwrap();
        assert_eq!(std::fs::read(fixture.path()).unwrap(), original);
        assert_eq!(
            std::fs::metadata(fixture.path()).unwrap().mode() & 0o777,
            0o600
        );
    }
    #[test]
    fn malformed_or_interrupted_first_records_are_never_replaced() {
        for bytes in [b"".as_slice(), b"{", b"null", b"{}", b"[]"] {
            let fixture = Fixture::new();
            std::fs::write(fixture.path(), bytes).unwrap();
            std::fs::set_permissions(fixture.path(), std::fs::Permissions::from_mode(0o600))
                .unwrap();
            assert!(fixture.record(SECOND).is_err());
            assert_eq!(std::fs::read(fixture.path()).unwrap(), bytes);
        }
    }
    #[test]
    fn missing_directory_or_invalid_identity_cannot_create_a_record() {
        let fixture = Fixture::new();
        for invocation in [
            "",
            "0",
            "1111111111111111111111111111111g",
            "00000000000000000000000000000000",
        ] {
            assert!(fixture.record(invocation).is_err());
            assert!(!fixture.path().exists());
        }
        assert!(
            record_first(&fixture.0.join("missing/first.json"), BOOT, FIRST, unsafe {
                libc::geteuid()
            })
            .is_err()
        );
        assert!(record_first(&fixture.path(), "bad-boot", FIRST, unsafe {
            libc::geteuid()
        })
        .is_err());
    }
    #[test]
    fn untrusted_permissions_ownership_and_hard_links_are_rejected() {
        let fixture = Fixture::new();
        fixture.record(FIRST).unwrap();
        assert!(record_first(
            &fixture.path(),
            BOOT,
            SECOND,
            unsafe { libc::geteuid() } + 1
        )
        .is_err());
        std::fs::set_permissions(fixture.path(), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(fixture.record(SECOND).is_err());
        std::fs::set_permissions(fixture.path(), std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::hard_link(fixture.path(), fixture.0.join("alias")).unwrap();
        assert!(fixture.record(SECOND).is_err());
        std::fs::remove_file(fixture.0.join("alias")).unwrap();
        std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(fixture.record(SECOND).is_err());
    }
    #[test]
    fn symlinks_and_nonregular_records_do_not_follow_or_block() {
        let fixture = Fixture::new();
        let target = fixture.0.join("target");
        std::fs::write(&target, "unchanged").unwrap();
        symlink(&target, fixture.path()).unwrap();
        assert!(fixture.record(FIRST).is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "unchanged");
        std::fs::remove_file(fixture.path()).unwrap();
        std::fs::create_dir(fixture.path()).unwrap();
        assert!(fixture.record(FIRST).is_err());
        std::fs::remove_dir(fixture.path()).unwrap();
        let filename = CString::new(fixture.path().as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(filename.as_ptr(), 0o600) }, 0);
        assert!(fixture.record(FIRST).is_err());
    }
    #[test]
    fn previous_boot_unknown_version_and_oversized_records_are_preserved() {
        for bytes in [
            format!(r#"{{"version":1,"boot_id":"different","invocation_id":"{FIRST}"}}"#),
            format!(r#"{{"version":2,"boot_id":"{BOOT}","invocation_id":"{FIRST}"}}"#),
            format!(r#"{{"version":1,"boot_id":"{BOOT}","invocation_id":"{FIRST}","extra":true}}"#),
            "x".repeat(4097),
        ] {
            let fixture = Fixture::new();
            std::fs::write(fixture.path(), &bytes).unwrap();
            std::fs::set_permissions(fixture.path(), std::fs::Permissions::from_mode(0o600))
                .unwrap();
            assert!(fixture.record(SECOND).is_err());
            assert_eq!(std::fs::read_to_string(fixture.path()).unwrap(), bytes);
        }
    }
}
