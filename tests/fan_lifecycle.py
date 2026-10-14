#!/usr/bin/env python3
"""Exercise the real daemon and systemd recovery using only fake fan files."""
import argparse
import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import tempfile
import time
import uuid

BUS = "org.shadowblip.PowerStation"
FAN = "/org/shadowblip/Performance/Fan/Fan0"
INTERFACE = "org.shadowblip.Fan.Device"
LOGIN = "org.freedesktop.login1"
LOGIN_PATH = "/org/freedesktop/login1"
CONTROL = "org.shadowblip.Fan.TestLogind"


def run(*arguments, check=True):
    result = subprocess.run(arguments, text=True, capture_output=True, timeout=40)
    if check and result.returncode:
        raise RuntimeError(f"{shlex.join(arguments)} failed: {result.stderr.strip()}")
    return result


def wait(predicate, description, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            if predicate():
                return
        except (RuntimeError, FileNotFoundError, ValueError):
            pass
        time.sleep(0.1)
    raise AssertionError(f"Timed out waiting for {description}")


class Harness:
    def __init__(self, root, binary, logind):
        self.root = root
        self.binary = binary
        self.logind = logind
        self.unit = f"powerstation-fan-test-{uuid.uuid4().hex}.service"
        self.processes = []
        self.logs = []
        self.assertions = 0
        self.started = False
        self.sysfs = root / "sys"
        self.platform = root / "platform"
        self.state = root / "state/fan-state.yaml"
        self.fan = self.sysfs / "class/hwmon/hwmon2"
        self.sensor = self.sysfs / "class/hwmon/hwmon9/temp1_input"

    def check(self, condition, message):
        if not condition:
            raise AssertionError(message)
        self.assertions += 1

    def write(self, relative, text):
        path = self.sysfs / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def bus(self, *arguments, check=True):
        return run("busctl", "--address", self.address, "--timeout=1", *arguments, check=check)

    def get(self, name):
        return shlex.split(self.bus("get-property", BUS, FAN, INTERFACE, name).stdout)[1]

    def call(self, name, signature=None, *arguments, check=True):
        args = ["call", BUS, FAN, INTERFACE, name]
        if signature is not None:
            args += [signature, *arguments]
        return self.bus(*args, check=check)

    def systemctl(self, *arguments, check=True):
        return run("systemctl", "--user", *arguments, self.unit, check=check)

    def pid(self):
        return int(self.systemctl("show", "--property=MainPID", "--value").stdout.strip())

    def start_logind(self):
        log = (self.root / f"logind-{len(self.logs)}.log").open("w")
        self.logs.append(log)
        process = subprocess.Popen([str(self.logind), self.address, str(self.root)], stdout=log, stderr=log)
        self.processes.append(process)
        wait(lambda: self.bus("get-property", LOGIN, LOGIN_PATH, CONTROL, "ActiveInhibitors", check=False).returncode == 0,
             "private logind fixture")
        return process

    def setup(self):
        self.platform.mkdir()
        source = Path(__file__).resolve().parents[1] / "rootfs/usr/share/powerstation/platform/fan-ayaneo-3.yaml"
        self.configuration = self.platform / source.name
        shutil.copyfile(source, self.configuration)
        self.original_yaml = self.configuration.read_text()
        for path, value in {
            "class/dmi/id/sys_vendor": "AYANEO\n",
            "class/dmi/id/product_name": "AYANEO 3\n",
            "class/hwmon/hwmon2/name": "ayaneo_ec\n",
            "class/hwmon/hwmon2/device/uevent": "DRIVER=ayaneo-ec\nDEVPATH=/devices/platform/ayaneo-ec\n",
            "class/hwmon/hwmon2/pwm1": "128\n",
            "class/hwmon/hwmon2/pwm1_enable": "2\n",
            "class/hwmon/hwmon2/fan1_input": "1200\n",
            "class/hwmon/hwmon9/name": "k10temp\n",
            "class/hwmon/hwmon9/temp1_label": "Tctl\n",
            "class/hwmon/hwmon9/temp1_input": "50000\n",
        }.items():
            self.write(path, value)
        bus_log = (self.root / "dbus.log").open("w")
        self.logs.append(bus_log)
        daemon = subprocess.Popen(["dbus-daemon", "--session", "--nofork", "--print-address=1"],
                                  stdout=subprocess.PIPE, stderr=bus_log, text=True)
        self.processes.append(daemon)
        self.address = daemon.stdout.readline().strip()
        if not self.address:
            raise RuntimeError("private D-Bus daemon did not start")
        self.login_process = self.start_logind()
        self.check(self.bus("call", LOGIN, LOGIN_PATH, "org.freedesktop.DBus.Properties",
                            "GetAll", "s", "org.freedesktop.login1.Manager", check=False).returncode != 0,
                   "fixture reproduces logind GetAll failure")
        self.check(self.bus("get-property", LOGIN, LOGIN_PATH, "org.freedesktop.login1.Manager",
                            "InhibitDelayMaxUSec", check=False).returncode == 0,
                   "inhibitor budget remains individually readable")

        unit_file = Path(__file__).resolve().parents[1] / "rootfs/usr/lib/systemd/system/powerstation.service"
        properties = []
        section = None
        for line in unit_file.read_text().splitlines():
            if line.startswith("["):
                section = line
            elif section == "[Service]" and "=" in line:
                key, value = line.split("=", 1)
                if key == "ExecStart":
                    continue
                if key == "ExecStopPost":
                    value = f"{self.binary} --restore-fans-auto"
                properties += [f"--property={key}={value}"]
        self.service_properties = properties
        self.start_service()
        self.ready()

    def start_service(self, check=True):
        # Inactive transient units are unloaded after a clean stop. Recreate
        # the same isolated service rather than starting an unloaded unit.
        self.systemctl("reset-failed", check=False)
        wait(lambda: self.systemctl("show", "--property=LoadState", "--value", check=False).stdout.strip() == "not-found",
             "previous transient unit to unload")
        self.started = True
        return run("systemd-run", "--user", "--quiet", f"--unit={self.unit}", *self.service_properties,
            f"--setenv=DBUS_SYSTEM_BUS_ADDRESS={self.address}",
            f"--setenv=POWERSTATION_FAN_PLATFORM_DIR={self.platform}",
            f"--setenv=POWERSTATION_FAN_SYSFS_ROOT={self.sysfs}",
            f"--setenv=POWERSTATION_FAN_STATE_PATH={self.state}", str(self.binary), check=check)

    def ready(self):
        wait(lambda: self.get("TelemetryValid") == "true", "validated fan telemetry")

    def mode(self, expected):
        wait(lambda: self.get("Mode") == expected, f"fan mode {expected}")

    def prepare(self, sleeping):
        outcome = self.root / "suspend-outcome"
        if sleeping:
            outcome.unlink(missing_ok=True)
        self.bus("call", LOGIN, LOGIN_PATH, CONTROL, "PrepareSleep", "b", str(sleeping).lower())
        if sleeping:
            wait(outcome.exists, "inhibitor release or expiry", timeout=5)
            return outcome.read_text()

    def exercise(self):
        self.check(self.get("Mode") == "automatic", "first use must be automatic")
        self.call("ApplyPreset", "s", "Quiet")
        self.check(self.get("Mode") == "preset", "Quiet selects preset mode")
        self.check(int(self.get("EffectivePercent")) == 10, "Quiet uses the running floor")
        self.call("SetManual", "y", "20")
        self.check((self.fan / "pwm1").read_text() == "51", "manual PWM must be verified")
        print("PASS initial Automatic, Quiet, and Manual", flush=True)

        saved = self.state.read_text()
        self.check(self.prepare(True) == "released", "successful handoff releases the inhibitor")
        self.check((self.fan / "pwm1_enable").read_text() == "2", "suspend hands control to firmware")
        self.check(self.get("Suspended") == "true", "suspend state is exposed")
        self.check(self.get("TelemetryValid") == "false", "suspended telemetry is invalid")
        for name, signature, args in [
            ("SetManual", "y", ["50"]),
            ("SetCurve", "a(dy)", ["2", "45", "0", "95", "100"]),
            ("ApplyPreset", "s", ["Quiet"]),
        ]:
            self.check(self.call(name, signature, *args, check=False).returncode != 0,
                       f"{name} must reject suspended custom control")
        self.check(self.state.read_text() == saved, "rejected calls preserve saved state")
        new_fan = self.fan.with_name("hwmon7")
        self.fan.rename(new_fan)
        self.fan = new_fan
        new_sensor = self.sensor.parent.with_name("hwmon11")
        self.sensor.parent.rename(new_sensor)
        self.sensor = new_sensor / "temp1_input"
        self.prepare(False)
        self.ready()
        self.mode("manual")
        self.check((self.fan / "pwm1_enable").read_text() == "1", "resume restores control at rediscovered paths")
        print("PASS suspend guards and resume with renumbered paths", flush=True)

        self.sensor.unlink()
        wait(lambda: self.get("TelemetryValid") == "false", "sensor loss invalidation")
        self.mode("automatic")
        self.check(self.get("Fault") != "", "sensor loss reports a fault")
        self.check(self.get("EffectivePercent") == "-1", "fault duty must be unavailable")
        self.call("SetAutomatic")
        self.check(self.get("AutomaticVerified") == "true", "Automatic works without telemetry")
        self.sensor.write_text("50000\n")
        self.ready()
        print("PASS sensor loss and telemetry recovery", flush=True)

        self.call("SetManual", "y", "20")
        self.systemctl("stop")
        self.check((self.fan / "pwm1_enable").read_text() == "2", "clean stop restores automatic")
        self.start_service()
        self.ready()
        self.mode("manual")
        self.check((self.fan / "pwm1_enable").read_text() == "1", "clean restart restores PowerStation's saved selection")
        print("PASS clean stop and saved-state restart", flush=True)

        for name, signum in [("SIGKILL", signal.SIGKILL), ("SIGSTOP/watchdog", signal.SIGSTOP)]:
            self.call("SetManual", "y", "20")
            old_pid = self.pid()
            os.kill(old_pid, signum)
            wait(lambda: self.pid() not in (0, old_pid), f"restart after {name}")
            self.ready()
            self.mode("automatic")
            self.check((self.fan / "pwm1_enable").read_text() == "2", f"{name} recovery restores automatic")
            self.check("mode: automatic" in self.state.read_text(), f"{name} persists Automatic")
            print(f"PASS {name} recovery", flush=True)

        self.call("SetManual", "y", "20")
        self.login_process.terminate()
        self.login_process.wait(timeout=5)
        wait(lambda: (self.fan / "pwm1_enable").read_text() == "2", "fallback after logind owner loss")
        # Firmware recovery precedes the atomic state-file update. Observe
        # both outcomes instead of racing persistence after the sysfs write.
        wait(lambda: "mode: automatic" in self.state.read_text(), "saved Automatic after logind owner loss")
        self.check("mode: automatic" in self.state.read_text(), "owner loss persists Automatic")
        self.systemctl("stop", check=False)
        self.login_process = self.start_logind()
        self.systemctl("reset-failed", check=False)
        self.start_service()
        self.ready()
        print("PASS logind owner loss", flush=True)

        self.call("SetManual", "y", "20")
        enable = self.fan / "pwm1_enable"
        enable.unlink()
        enable.mkdir()
        self.check(self.prepare(True) in ("released", "expired"), "failed handoff cannot indefinitely delay sleep")
        result = self.call("SetManual", "y", "50", check=False)
        self.check(result.returncode != 0, "failed handoff must not admit custom control")
        enable.rmdir()
        enable.write_text("2\n")
        self.systemctl("stop", check=False)
        self.systemctl("reset-failed", check=False)
        self.prepare(False)
        self.start_service()
        self.ready()
        print("PASS bounded failed handoff", flush=True)

        self.systemctl("stop")
        self.configuration.write_text("not: [valid\n")
        self.start_service(check=False)
        wait(lambda: self.systemctl("show", "--property=ExecMainStatus", "--value").stdout.strip() == "1",
             "malformed YAML startup failure")
        self.check(self.call("SetManual", "y", "50", check=False).returncode != 0,
                   "malformed configuration must not expose a healthy fan service")
        self.systemctl("stop", check=False)
        self.configuration.write_text(self.original_yaml)
        print("PASS malformed configuration rejection", flush=True)
        print(f"PASS lifecycle harness: {self.assertions} assertions", flush=True)

    def close(self):
        if self.started:
            self.systemctl("stop", check=False)
            self.systemctl("reset-failed", check=False)
        for process in reversed(self.processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
        for log in self.logs:
            log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=lambda value: Path(value).resolve())
    parser.add_argument("logind", type=lambda value: Path(value).resolve())
    arguments = parser.parse_args()
    for command in ("systemd-run", "systemctl", "busctl", "dbus-daemon"):
        if shutil.which(command) is None:
            parser.error(f"missing command: {command}")
    for binary in (arguments.binary, arguments.logind):
        if not binary.is_file():
            parser.error(f"missing built binary: {binary}")
    run("systemctl", "--user", "list-units", "--no-pager", "--no-legend")
    with tempfile.TemporaryDirectory(prefix="powerstation-fan-lifecycle-") as directory:
        harness = Harness(Path(directory), arguments.binary, arguments.logind)
        try:
            harness.setup()
            harness.exercise()
        except Exception:
            if harness.started:
                result = run("journalctl", "--user", "--unit", harness.unit, "--no-pager", "-n", "30", check=False)
                print(result.stdout, flush=True)
            raise
        finally:
            harness.close()


if __name__ == "__main__":
    main()
