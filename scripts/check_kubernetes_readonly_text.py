#!/usr/bin/env python3
"""Reject mutating kubectl commands outside integration-test fixtures."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SELF_PATH = Path("scripts/check_kubernetes_readonly_text.py")

SCAN_PATHS = (
    ".github",
    "README.md",
    "Dockerfile",
    "scripts",
    "src",
)

EXCLUDED_PATHS = {
    SELF_PATH,
}

EXCLUDED_PREFIXES = (
    Path("tests/kind_integration"),
)

TEXT_SUFFIXES = {
    "",
    ".bash",
    ".dockerfile",
    ".md",
    ".py",
    ".rs",
    ".sh",
    ".toml",
    ".yaml",
    ".yml",
}

MUTATING_VERBS = (
    "annotate",
    "apply",
    "attach",
    "auth\\s+reconcile",
    "autoscale",
    "certificate\\s+approve",
    "certificate\\s+deny",
    "cordon",
    "create",
    "debug",
    "delete",
    "drain",
    "edit",
    "exec",
    "expose",
    "label",
    "patch",
    "port-forward",
    "replace",
    r"rollout\s+pause",
    r"rollout\s+resume",
    r"rollout\s+restart",
    r"rollout\s+undo",
    "run",
    "scale",
    "set",
    "taint",
    "uncordon",
)

MUTATING_KUBECTL = re.compile(
    rf"""
    (?P<command>\bkubectl\b[^\n;&|]*?
    \b(?:
        {"|".join(MUTATING_VERBS)}
    )\b[^\n;&|]*)
    """,
    re.IGNORECASE | re.VERBOSE,
)


@dataclass(frozen=True)
class NormalizedText:
    text: str
    original_text: str
    original_offsets: list[int]

    def original_line_for(self, normalized_offset: int) -> int:
        original_offset = self.original_offsets[normalized_offset]
        return self.original_text.count("\n", 0, original_offset) + 1


@dataclass(frozen=True)
class Finding:
    path: Path
    line: int
    command: str


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run rule self-tests instead of scanning repository files.",
    )
    args = parser.parse_args()

    if args.self_test:
        return run_self_test()

    findings = scan_repository()
    if not findings:
        return 0

    report_findings(findings)
    return 1


def report_findings(findings: list[Finding]) -> None:
    print("Kubernetes read-only text guard failed:", file=sys.stderr)
    for finding in findings:
        print(
            f"{finding.path}:{finding.line}: mutating kubectl command is not allowed: "
            f"{finding.command}",
            file=sys.stderr,
        )
    print(
        "Production paths must not create, update, delete, exec, attach, port-forward, "
        "restart, or scale Kubernetes resources.",
        file=sys.stderr,
    )


def scan_repository() -> list[Finding]:
    findings: list[Finding] = []
    for path in iter_scan_files():
        rel_path = path.relative_to(ROOT)
        for line_number, command in find_mutating_commands(read_text(path)):
            findings.append(
                Finding(
                    path=rel_path,
                    line=line_number,
                    command=command,
                )
            )
    return findings


def find_mutating_commands(text: str) -> list[tuple[int, str]]:
    normalized = fold_shell_continuations(text)
    commands = []
    for match in MUTATING_KUBECTL.finditer(normalized.text):
        command = " ".join(match.group("command").split())
        if is_read_only_kubectl_command(command):
            continue
        commands.append(
            (
                normalized.original_line_for(match.start()),
                command,
            )
        )
    return commands


def is_read_only_kubectl_command(command: str) -> bool:
    return re.match(r"(?i)^kubectl\s+auth\s+can-i\b", command) is not None


def fold_shell_continuations(text: str) -> NormalizedText:
    normalized = []
    original_offsets = []
    index = 0
    while index < len(text):
        if text[index] == "\\" and index + 1 < len(text) and text[index + 1] in "\r\n":
            normalized.append(" ")
            original_offsets.append(index)
            index += 1
            if index < len(text) and text[index] == "\r":
                index += 1
            if index < len(text) and text[index] == "\n":
                index += 1
            while index < len(text) and text[index] in " \t":
                index += 1
            continue

        normalized.append(text[index])
        original_offsets.append(index)
        index += 1

    return NormalizedText("".join(normalized), text, original_offsets)


def iter_scan_files() -> list[Path]:
    files: list[Path] = []
    for scan_path in SCAN_PATHS:
        path = ROOT / scan_path
        if path.is_file():
            candidates = [path]
        elif path.is_dir():
            candidates = [candidate for candidate in path.rglob("*") if candidate.is_file()]
        else:
            candidates = []
        files.extend(candidate for candidate in candidates if should_scan(candidate))
    return sorted(files)


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def should_scan(path: Path) -> bool:
    rel_path = path.relative_to(ROOT)
    if rel_path in EXCLUDED_PATHS:
        return False
    if any(rel_path == prefix or prefix in rel_path.parents for prefix in EXCLUDED_PREFIXES):
        return False
    return path.suffix.lower() in TEXT_SUFFIXES or path.name == "Dockerfile"


def run_self_test() -> int:
    failures: list[str] = []
    failures.extend(check_allowed_commands())
    failures.extend(check_denied_commands())
    failures.extend(check_line_number_mapping())
    failures.extend(check_path_filters())

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    return 0


def check_allowed_commands() -> list[str]:
    allowed = (
        "kubectl get pods",
        "kubectl describe deployment web",
        "kubectl logs web-0 --tail=100",
        "kubectl cluster-info",
        "kubectl version --client",
        "kubectl auth can-i create pods",
        "kubectl auth can-i delete pods --namespace default",
    )
    return [
        f"allowed command was rejected: {command}"
        for command in allowed
        if find_mutating_commands(command)
    ]


def check_denied_commands() -> list[str]:
    denied = (
        "kubectl apply -f deploy.yaml",
        "kubectl \\\n  apply -f deploy.yaml",
        "kubectl auth reconcile -f rbac.yaml",
        "kubectl autoscale deployment web --min=1 --max=3",
        "kubectl create namespace demo",
        "kubectl debug pod/web-0 -it --image=busybox",
        "kubectl delete pod web-0",
        "kubectl \\\n  delete pod web-0",
        "kubectl annotate pod web-0 owner=platform",
        "kubectl label pod web-0 tier=frontend",
        "kubectl cordon node-1",
        "kubectl drain node-1",
        "kubectl edit deployment web",
        "kubectl exec web-0 -- sh",
        "kubectl rollout restart deployment/web",
        "kubectl rollout undo deployment/web",
        "kubectl rollout pause deployment/web",
        "kubectl rollout resume deployment/web",
        "kubectl port-forward pod/web-0 8080:80",
        "kubectl scale deployment/web --replicas=0",
        "kubectl taint nodes node-1 dedicated=ci:NoSchedule",
        "kubectl uncordon node-1",
        "kubectl certificate approve csr-1",
        "kubectl certificate deny csr-1",
    )
    return [
        f"denied command was accepted: {command}"
        for command in denied
        if not find_mutating_commands(command)
    ]


def check_line_number_mapping() -> list[str]:
    multiline = "echo before\nkubectl \\\n  delete pod web-0\n"
    if find_mutating_commands(multiline) == [(2, "kubectl delete pod web-0")]:
        return []
    return ["backslash-continued kubectl commands should preserve line numbers"]


def check_path_filters() -> list[str]:
    failures = []
    if should_scan(ROOT / "tests/kind_integration/cluster.rs"):
        failures.append("kind integration helpers should be excluded")
    if should_scan(ROOT / SELF_PATH):
        failures.append("the guard script should not scan itself")
    return failures


if __name__ == "__main__":
    raise SystemExit(main())
