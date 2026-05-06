use std::io::Write as _;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

const FIXTURE: &str = include_str!("fixtures/prod_smoke.yaml.tmpl");

pub(crate) struct NamespaceGuard {
    name: String,
}

impl NamespaceGuard {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Drop for NamespaceGuard {
    fn drop(&mut self) {
        let _ = Command::new("kubectl")
            .args(["delete", "namespace", &self.name, "--ignore-not-found=true"])
            .status();
    }
}

pub(crate) fn apply_fixture(namespace: &str) -> anyhow::Result<()> {
    let manifest = FIXTURE.replace("__NAMESPACE__", namespace);
    kubectl_stdin(&["apply", "-f", "-"], &manifest)
}

pub(crate) fn wait_for_workloads(namespace: &str) -> anyhow::Result<()> {
    kubectl(&[
        "rollout",
        "status",
        "deployment/frontend",
        "-n",
        namespace,
        "--timeout=180s",
    ])?;
    kubectl(&[
        "rollout",
        "status",
        "statefulset/cache",
        "-n",
        namespace,
        "--timeout=180s",
    ])?;
    kubectl(&[
        "rollout",
        "status",
        "daemonset/node-probe",
        "-n",
        namespace,
        "--timeout=180s",
    ])?;
    kubectl(&[
        "wait",
        "--for=condition=complete",
        "job/complete-once",
        "-n",
        namespace,
        "--timeout=120s",
    ])?;
    kubectl(&[
        "wait",
        "--for=condition=failed",
        "job/fail-once",
        "-n",
        namespace,
        "--timeout=120s",
    ])?;

    let _ = Command::new("kubectl")
        .args([
            "wait",
            "--for=condition=Ready",
            "pod",
            "-l",
            "app=broken-api",
            "-n",
            namespace,
            "--timeout=5s",
        ])
        .output();
    thread::sleep(Duration::from_secs(5));
    Ok(())
}

fn kubectl(args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("kubectl").args(args).output()?;
    if output.status.success() {
        return Ok(());
    }

    anyhow::bail!(
        "kubectl {} failed\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn kubectl_stdin(args: &[&str], stdin: &str) -> anyhow::Result<()> {
    let mut child = Command::new("kubectl")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child.stdin.take().unwrap().write_all(stdin.as_bytes())?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        return Ok(());
    }

    anyhow::bail!(
        "kubectl {} failed\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
