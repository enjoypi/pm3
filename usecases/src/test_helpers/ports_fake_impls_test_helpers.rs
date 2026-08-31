use super::*;

#[expect(
    clippy::unused_async_trait_impl,
    reason = "桩实现直接返回既有值，无 await；改 impl Future + ready 会让测试夹具难读"
)]
impl ProcessLauncher for FakePorts {
    async fn spawn(&self, spec: &LaunchSpec) -> Result<LaunchedProcess, LaunchError> {
        self.record_spawn(spec)
    }

    async fn adopt(&self, pid: u32) {
        self.with_state(|state| {
            state.adopted.push(pid);
            state.tracked.insert(pid);
        });
    }

    async fn tracked_pids(&self) -> Vec<u32> {
        self.read(|state| state.tracked.iter().copied().collect())
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "桩实现直接返回既有值，无 await；改 impl Future + ready 会让测试夹具难读"
)]
impl Signaler for FakePorts {
    async fn terminate(&self, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        self.record_signal(pid, scope)
    }

    async fn force_kill(&self, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        self.record_force_kill(pid, scope)
    }

    async fn deliver(&self, signal: &str, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        self.record_deliver(signal, pid, scope)
    }
}

impl CommandWrapper for FakePorts {
    fn wrap(
        &self,
        app: &str,
        policy: &SandboxPolicy,
        program: &str,
        args: &[String],
    ) -> Result<WrappedCommand, SandboxError> {
        if self.read(|state| state.wrap_failures.iter().any(|name| name == app)) {
            return Err(SandboxError::NoBackend {
                app: app.to_string(),
            });
        }
        if policy.mode.is_unconfined() {
            return Ok(WrappedCommand {
                program: program.to_string(),
                args: args.to_vec(),
            });
        }
        let mut wrapped_args = vec![program.to_string()];
        wrapped_args.extend_from_slice(args);
        Ok(WrappedCommand {
            program: SANDBOX_PREFIX.to_string(),
            args: wrapped_args,
        })
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "桩实现直接返回既有值，无 await；改 impl Future + ready 会让测试夹具难读"
)]
impl DumpStore for FakePorts {
    async fn load(&self) -> Result<DumpContents, DumpError> {
        self.read_stored()
    }

    async fn save(&self, records: &[ProcessRecord], boot: Option<&str>) -> Result<(), DumpError> {
        self.record_save(records, boot)
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "桩实现直接返回既有值，无 await；改 impl Future + ready 会让测试夹具难读"
)]
impl LogRotator for FakePorts {
    async fn rotate_logs(
        &self,
        _logs_dir: &str,
        _max_bytes: u64,
    ) -> Result<Vec<RotatedLog>, LogRotateError> {
        Ok(Vec::new())
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "桩实现直接返回既有值，无 await；改 impl Future + ready 会让测试夹具难读"
)]
impl ReadyProber for FakePorts {
    async fn check_ready(&self, _probe: &ReadyProbe) -> Readiness {
        Readiness::Ready
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "桩实现直接返回既有值，无 await；改 impl Future + ready 会让测试夹具难读"
)]
impl ProcessProbe for FakePorts {
    async fn resident_memory(&self, _pids: &[u32]) -> BTreeMap<u32, u64> {
        BTreeMap::new()
    }

    async fn resource_usage(&self, pids: &[u32]) -> BTreeMap<u32, ResourceSample> {
        self.read(|state| {
            pids.iter()
                .filter_map(|pid| state.resources.get(pid).map(|sample| (*pid, *sample)))
                .collect()
        })
    }

    async fn identity(&self, pid: u32) -> Liveness {
        self.identities(&[pid])
            .await
            .remove(&pid)
            .unwrap_or(Liveness::Unreadable)
    }

    async fn identities(&self, pids: &[u32]) -> HashMap<u32, Liveness> {
        let mut guard = self.locked();
        let observed: HashMap<u32, Liveness> = pids
            .iter()
            .map(|pid| {
                let liveness = if guard.probe_broken.contains(pid) {
                    Liveness::Unreadable
                } else if guard.probed.contains(pid) && guard.vanished_after_probe.contains(pid) {
                    Liveness::Gone
                } else if guard.probed.contains(pid) && guard.recycled_after_probe.contains(pid) {
                    Liveness::Alive(format!("{RECYCLED_TOKEN_PREFIX}{pid}"))
                } else {
                    guard
                        .live
                        .get(pid)
                        .cloned()
                        .map_or(Liveness::Gone, Liveness::Alive)
                };
                (*pid, liveness)
            })
            .collect();
        guard.probed.extend(pids.iter().copied());
        observed
    }

    async fn wait_gone(&self, pid: u32, timeout_ms: u64) -> Liveness {
        let _ = timeout_ms;
        if self.read(|state| state.slow_wait) {
            tokio::task::yield_now().await;
        }
        self.with_state(|state| {
            state.waited.push(pid);
            state.events.push(format!("wait:{pid}"));
        });
        self.identity(pid).await
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "桩实现直接返回既有值，无 await；改 impl Future + ready 会让测试夹具难读"
)]
impl Fingerprinter for FakePorts {
    fn digest(&self, text: &str) -> String {
        format!("{TEXT_DIGEST_PREFIX}{text}")
    }

    async fn file_digest(&self, path: &str) -> Result<String, FingerprintError> {
        let digest = {
            let guard = self.locked();
            if guard.digest_failures.iter().any(|failed| failed == path) {
                return Err(FingerprintError::Read {
                    path: path.to_string(),
                    reason: "injected digest failure".to_string(),
                });
            }
            guard
                .file_digests
                .get(path)
                .cloned()
                .unwrap_or_else(|| format!("{FILE_DIGEST_PREFIX}{path}"))
        };
        Ok(digest)
    }
}

impl Ports for FakePorts {}

impl Clock for FakePorts {
    fn now_ms(&self) -> u64 {
        self.read(|state| state.now_ms)
    }
}

impl Scheduler for FakePorts {
    fn next_fire_ms(&self, cron: &str, after_ms: u64) -> Option<u64> {
        if cron == UNSCHEDULABLE_CRON {
            return None;
        }
        Some(after_ms.saturating_add(FAKE_FIRE_INTERVAL_MS))
    }
}
