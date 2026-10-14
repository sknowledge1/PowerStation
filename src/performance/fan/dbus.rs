use super::config::CurvePoint;
use super::controller::{ControllerError, FanController, FanSnapshot};
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::fdo;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::ObjectPath;
use zbus_macros::interface;

pub struct FanBus {
    object_paths: Vec<String>,
}

impl FanBus {
    pub fn new(object_paths: Vec<String>) -> Self {
        Self { object_paths }
    }
}

#[interface(name = "org.shadowblip.Fan")]
impl FanBus {
    /// Return fan object paths selected by the platform database; an unsupported system returns an empty list.
    pub async fn enumerate_fans(&self) -> fdo::Result<Vec<ObjectPath>> {
        Ok(self
            .object_paths
            .iter()
            .map(|path| ObjectPath::from_string_unchecked(path.clone()))
            .collect())
    }
}

#[derive(Clone)]
pub struct FanDevice {
    pub controller: Arc<Mutex<FanController>>,
}

impl FanDevice {
    pub fn new(controller: Arc<Mutex<FanController>>) -> Self {
        Self { controller }
    }

    async fn snapshot(&self) -> FanSnapshot {
        self.controller.lock().await.snapshot()
    }

    async fn update<F>(&self, emitter: &SignalEmitter<'_>, operation: F) -> fdo::Result<()>
    where
        F: FnOnce(&mut FanController) -> Result<(), ControllerError>,
    {
        let result = {
            let mut controller = self.controller.lock().await;
            operation(&mut controller).map_err(|error| fdo::Error::Failed(error.to_string()))
        };
        // Failed operations can enter automatic fallback. Notify clients even
        // on that path, retaining the original error if notification also fails.
        let notification = self.emit_configuration_changes(emitter).await;
        result.and(notification)
    }

    pub async fn emit_all_changes(&self, emitter: &SignalEmitter<'_>) -> zbus::Result<()> {
        self.mode_changed(emitter).await?;
        self.backend_changed(emitter).await?;
        self.temperature_c_changed(emitter).await?;
        self.telemetry_valid_changed(emitter).await?;
        self.rpm_changed(emitter).await?;
        self.effective_percent_changed(emitter).await?;
        self.manual_percent_changed(emitter).await?;
        self.curve_changed(emitter).await?;
        self.custom_curve_changed(emitter).await?;
        self.active_preset_changed(emitter).await?;
        self.fault_changed(emitter).await?;
        self.automatic_verified_changed(emitter).await?;
        self.controlling_changed(emitter).await?;
        self.suspended_changed(emitter).await?;
        self.curve_point_count_changed(emitter).await?;
        Ok(())
    }

    async fn emit_configuration_changes(&self, emitter: &SignalEmitter<'_>) -> fdo::Result<()> {
        self.emit_all_changes(emitter)
            .await
            .map_err(|error| fdo::Error::Failed(error.to_string()))
    }
}

#[interface(name = "org.shadowblip.Fan.Device")]
impl FanDevice {
    /// Restore and verify firmware automatic mode, then persist that selection. Available during suspend and faults without temperature or RPM telemetry.
    pub async fn set_automatic(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.update(&emitter, FanController::set_automatic).await
    }

    /// Select and persist a manual percentage within the platform limits. The running floor and thermal overrides can raise the effective duty. Fails without changing the selection while lifecycle protection is unavailable.
    pub async fn set_manual(
        &self,
        percent: u8,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.update(&emitter, |controller| controller.set_manual(percent))
            .await
    }

    /// Select and persist increasing temperature/percentage points ending at 100 percent. Hardware curves require exactly CurvePointCount points; software curves accept two or more. Fails during lifecycle transitions.
    pub async fn set_curve(
        &self,
        points: Vec<(f64, u8)>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let points = points
            .into_iter()
            .map(|(temperature_c, percent)| CurvePoint {
                temperature_c,
                percent,
            })
            .collect();
        self.update(&emitter, |controller| controller.set_curve(points))
            .await
    }

    /// Select and persist a named platform preset. Fails for unknown names or while lifecycle protection is unavailable.
    pub async fn apply_preset(
        &self,
        name: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.update(&emitter, |controller| controller.apply_preset(name))
            .await
    }

    /// Version of the fan D-Bus contract; currently 1.
    #[zbus(property)]
    pub async fn api_version(&self) -> u32 {
        1
    }

    /// Stable fan identifier within the selected platform.
    #[zbus(property)]
    pub async fn id(&self) -> String {
        self.snapshot().await.id
    }

    /// Human-readable fan name from the platform database.
    #[zbus(property)]
    pub async fn name(&self) -> String {
        self.snapshot().await.name
    }

    /// Persisted selection: automatic, manual, curve, or preset. During suspend the selected mode is retained while firmware controls the fan.
    #[zbus(property)]
    pub async fn mode(&self) -> String {
        self.snapshot().await.mode
    }

    /// Curve backend: software_curve or hardware_curve. Manual mode always uses the monitored software policy.
    #[zbus(property)]
    pub async fn backend(&self) -> String {
        self.snapshot().await.backend
    }

    /// Supported selections, independent of temporary lifecycle restrictions. Includes preset only when Presets is nonempty.
    #[zbus(property)]
    pub async fn available_modes(&self) -> Vec<String> {
        let snapshot = self.snapshot().await;
        let mut modes = vec!["automatic".into(), "manual".into(), "curve".into()];
        if !snapshot.presets.is_empty() {
            modes.push("preset".into());
        }
        modes
    }

    /// Hottest configured sensor reading in degrees Celsius. Returns 0 when TelemetryValid is false.
    #[zbus(property)]
    pub async fn temperature_c(&self) -> f64 {
        self.snapshot().await.temperature_c.unwrap_or(0.0)
    }

    /// Whether a complete trusted temperature/RPM sample is available. False after faults and during suspend until fresh telemetry is validated.
    #[zbus(property)]
    pub async fn telemetry_valid(&self) -> bool {
        let snapshot = self.snapshot().await;
        snapshot.temperature_c.is_some() && snapshot.rpm.is_some()
    }

    /// Last validated fan speed in RPM. Returns 0 when TelemetryValid is false.
    #[zbus(property)]
    pub async fn rpm(&self) -> u32 {
        self.snapshot().await.rpm.unwrap_or(0)
    }

    /// Applied software duty or estimated hardware-curve duty, including driver quantization tolerance. Returns -1 when unavailable or firmware owns control.
    #[zbus(property)]
    pub async fn effective_percent(&self) -> i16 {
        self.snapshot()
            .await
            .effective_percent
            .map(i16::from)
            .unwrap_or(-1)
    }

    /// Saved manual target percentage, retained when another mode is selected.
    #[zbus(property)]
    pub async fn manual_percent(&self) -> u8 {
        self.snapshot().await.manual_percent
    }

    /// Selected custom or preset curve as temperature/percentage pairs. A saved custom curve can remain visible in automatic or manual mode.
    #[zbus(property)]
    pub async fn curve(&self) -> Vec<(f64, u8)> {
        self.snapshot()
            .await
            .curve
            .into_iter()
            .map(|point| (point.temperature_c, point.percent))
            .collect()
    }

    /// Saved custom curve, retained independently of the selected preset. Empty before the first custom curve is saved.
    #[zbus(property)]
    pub async fn custom_curve(&self) -> Vec<(f64, u8)> {
        self.snapshot()
            .await
            .custom_curve
            .into_iter()
            .map(|point| (point.temperature_c, point.percent))
            .collect()
    }

    /// Available preset names from the platform database.
    #[zbus(property)]
    pub async fn presets(&self) -> Vec<String> {
        self.snapshot().await.presets
    }

    /// Selected preset name, or an empty string outside preset mode.
    #[zbus(property)]
    pub async fn active_preset(&self) -> String {
        self.snapshot().await.active_preset.unwrap_or_default()
    }

    /// Last fan fault, or an empty string after successful configuration. Fresh telemetry alone does not acknowledge an earlier fault.
    #[zbus(property)]
    pub async fn fault(&self) -> String {
        self.snapshot().await.fault.unwrap_or_default()
    }

    /// Whether the most recent ownership read or automatic-mode write verified firmware automatic mode.
    #[zbus(property)]
    pub async fn automatic_verified(&self) -> bool {
        self.snapshot().await.automatic_verified
    }

    /// Whether PowerStation currently owns manual or hardware-curve control.
    #[zbus(property)]
    pub async fn controlling(&self) -> bool {
        self.snapshot().await.controlling
    }

    /// Whether the controller is paused for suspend or shutdown; custom selections are rejected while true.
    #[zbus(property)]
    pub async fn suspended(&self) -> bool {
        self.snapshot().await.suspended
    }

    /// Required native point count for hardware curves; 0 for software curves.
    #[zbus(property)]
    pub async fn curve_point_count(&self) -> u16 {
        self.snapshot().await.curve_point_count
    }

    /// Minimum accepted manual or curve percentage.
    #[zbus(property)]
    pub async fn minimum_percent(&self) -> u8 {
        self.controller.lock().await.config.limits.minimum_percent
    }

    /// Minimum software duty while the fan is running.
    #[zbus(property)]
    pub async fn minimum_running_percent(&self) -> u8 {
        self.controller
            .lock()
            .await
            .config
            .limits
            .minimum_running_percent
    }

    /// Raw PWM value representing 0 percent.
    #[zbus(property)]
    pub async fn pwm_min(&self) -> u32 {
        self.controller.lock().await.config.limits.pwm_min
    }

    /// Raw PWM value representing 100 percent.
    #[zbus(property)]
    pub async fn pwm_max(&self) -> u32 {
        self.controller.lock().await.config.limits.pwm_max
    }

    /// Maximum permitted raw PWM readback difference caused by driver quantization.
    #[zbus(property)]
    pub async fn pwm_readback_tolerance(&self) -> u32 {
        self.controller
            .lock()
            .await
            .config
            .limits
            .pwm_readback_tolerance
    }

    /// Software fan-stop threshold in degrees Celsius when a zero-percent target is permitted.
    #[zbus(property)]
    pub async fn stop_temperature_c(&self) -> f64 {
        self.controller
            .lock()
            .await
            .config
            .limits
            .stop_temperature_c
    }

    /// Software fan restart threshold in degrees Celsius; restart occurs above this value.
    #[zbus(property)]
    pub async fn restart_temperature_c(&self) -> f64 {
        self.controller
            .lock()
            .await
            .config
            .limits
            .restart_temperature_c
    }

    /// RPM below which a running fan is considered stalled after StallGraceSeconds.
    #[zbus(property)]
    pub async fn stall_rpm(&self) -> u32 {
        self.controller.lock().await.config.limits.stall_rpm
    }

    /// Number of monitored one-second intervals allowed below StallRpm before automatic fallback.
    #[zbus(property)]
    pub async fn stall_grace_seconds(&self) -> u32 {
        self.controller
            .lock()
            .await
            .config
            .limits
            .stall_grace_seconds
    }

    /// Temperature at which software control forces 100 percent; every accepted curve reaches 100 percent by this threshold.
    #[zbus(property)]
    pub async fn full_speed_temperature_c(&self) -> f64 {
        self.controller
            .lock()
            .await
            .config
            .limits
            .full_speed_temperature_c
    }

    /// Temperature at which custom control is abandoned for verified firmware automatic mode.
    #[zbus(property)]
    pub async fn emergency_handoff_temperature_c(&self) -> f64 {
        self.controller
            .lock()
            .await
            .config
            .limits
            .emergency_handoff_temperature_c
    }

    /// Lowest accepted telemetry temperature in degrees Celsius.
    #[zbus(property)]
    pub async fn minimum_valid_temperature_c(&self) -> f64 {
        self.controller
            .lock()
            .await
            .config
            .limits
            .minimum_valid_temperature_c
    }

    /// Highest accepted telemetry temperature in degrees Celsius.
    #[zbus(property)]
    pub async fn maximum_valid_temperature_c(&self) -> f64 {
        self.controller
            .lock()
            .await
            .config
            .limits
            .maximum_valid_temperature_c
    }

    /// Highest accepted fan RPM; greater readings invalidate telemetry.
    #[zbus(property)]
    pub async fn maximum_valid_rpm(&self) -> u32 {
        self.controller.lock().await.config.limits.maximum_valid_rpm
    }
}
