#!/usr/bin/env python3
"""Safely collect and redact explicitly allowlisted Phase A2 text evidence."""

from __future__ import annotations

import hashlib
import os
import re
import stat
import sys
from pathlib import Path

sys.dont_write_bytecode = True

DESTINATION_RE = re.compile(r"^[0-9]{8}T[0-9]{6}Z-(?:path-[abc]|common)$")
KIND_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")
CREDENTIAL_NAME_RE = re.compile(
    r"openrc|credential|secret|private.?key|\.pem$|\.p12$|\.pfx$|clouds\.yaml|cookie|token",
    re.IGNORECASE,
)
SECRET_RE = re.compile(
    r"((?:password|passwd|secret|token|api[_-]?key|auth[_-]?key)\s*[:=]\s*)[^\s,;]+",
    re.IGNORECASE,
)
JSON_SECRET_RE = re.compile(
    r'("(?:password|passwd|secret|token|api[_-]?key|auth[_-]?key)"\s*:\s*")[^"]*(")',
    re.IGNORECASE,
)
AUTH_RE = re.compile(r"(Authorization:\s*(?:Bearer|Basic))\s+\S+", re.IGNORECASE)
USERINFO_RE = re.compile(r"(https?://)[^/@\s]+:[^/@\s]+@")
ALLOWED_INPUT_KEYS = (
    "CELLHV_LAB_ID",
    "CELLHV_LAB_CREDENTIAL_CLASS",
    "CELLHV_RESOURCE_PREFIX",
    "CELLHV_HOST_DISTRIBUTION",
    "CELLHV_ARCHITECTURE",
    "CELLHV_OPENSTACK_RELEASE",
    "CELLHV_DEVSTACK_COMMIT",
    "CELLHV_NOVA_COMMIT",
    "CELLHV_LIBVIRT_VERSION",
    "CELLHV_CLOUD_HYPERVISOR_VERSION",
    "CELLHV_CLOUD_HYPERVISOR_SHA256",
    "CELLHV_GUEST_IMAGE_NAME",
    "CELLHV_GUEST_IMAGE_SHA256",
    "CELLHV_OVMF_PACKAGE_VERSION",
)
MAX_SOURCE_BYTES = 100 * 1024 * 1024


def fail(message: str) -> "NoReturn":
    raise SystemExit(f"[FAIL] {message}")


def open_regular_nofollow(path: Path) -> tuple[int, os.stat_result]:
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    except OSError as error:
        fail(f"cannot safely open regular file {path}: {error}")
    details = os.fstat(descriptor)
    if not stat.S_ISREG(details.st_mode):
        os.close(descriptor)
        fail(f"source is not a regular file: {path}")
    return descriptor, details


def read_text_once(path: Path) -> str:
    descriptor, details = open_regular_nofollow(path)
    try:
        if details.st_size > MAX_SOURCE_BYTES:
            fail(f"source exceeds {MAX_SOURCE_BYTES} bytes: {path}")
        with os.fdopen(descriptor, "rb", closefd=True) as stream:
            data = stream.read(MAX_SOURCE_BYTES + 1)
            descriptor = -1
        if len(data) > MAX_SOURCE_BYTES or b"\0" in data:
            fail(f"source is not bounded plain text: {path}")
        try:
            return data.decode("utf-8")
        except UnicodeDecodeError:
            fail(f"source is not UTF-8 text: {path}")
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def parse_inputs(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in read_text_once(path).splitlines():
        if not line or line.startswith("#"):
            continue
        key, separator, value = line.partition("=")
        if not separator or key not in ALLOWED_INPUT_KEYS or key in values:
            fail(f"unexpected or duplicate lab input key: {key}")
        values[key] = value
    if set(values) != set(ALLOWED_INPUT_KEYS):
        fail("lab inputs do not contain the exact safe key set")
    return values


def parse_allowlist(path: Path) -> list[tuple[str, Path]]:
    entries: list[tuple[str, Path]] = []
    for line in read_text_once(path).splitlines():
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 2:
            fail("allowlist rows require exactly two tab-separated fields")
        kind, source_text = fields
        if not KIND_RE.fullmatch(kind):
            fail(f"invalid evidence kind: {kind}")
        if "\n" in source_text or "\r" in source_text:
            fail("evidence path contains a line break")
        source = Path(source_text)
        if not source.is_absolute():
            fail(f"evidence source must be absolute: {source}")
        if CREDENTIAL_NAME_RE.search(source.name):
            fail(f"credential-like evidence source is forbidden: {source}")
        entries.append((kind, source))
    if not entries:
        fail("evidence allowlist is empty")
    return entries


def verify_parent(path: Path) -> tuple[Path, int]:
    parent = path.parent
    current = Path(parent.anchor or ".")
    for component in parent.parts[1:] if parent.is_absolute() else parent.parts:
        current /= component
        try:
            if stat.S_ISLNK(os.lstat(current).st_mode):
                fail(f"destination parent chain contains a symlink: {current}")
        except OSError as error:
            fail(f"destination parent must already exist: {error}")
    try:
        parent_details = os.lstat(parent)
    except OSError as error:
        fail(f"destination parent must already exist: {error}")
    if not stat.S_ISDIR(parent_details.st_mode) or stat.S_ISLNK(parent_details.st_mode):
        fail("destination parent must be a real directory, not a symlink")
    if parent_details.st_uid not in {os.geteuid(), 0}:
        fail("destination parent must be owned by the current user or root")
    if parent_details.st_mode & stat.S_IWOTH and not parent_details.st_mode & stat.S_ISVTX:
        fail("destination parent is untrusted: world-writable without sticky bit")
    descriptor = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW)
    opened = os.fstat(descriptor)
    if (opened.st_dev, opened.st_ino) != (parent_details.st_dev, parent_details.st_ino):
        os.close(descriptor)
        fail("destination parent changed during validation")
    return parent, descriptor


def write_file(directory_fd: int, relative: str, content: bytes) -> None:
    descriptor = os.open(
        relative,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW,
        0o600,
        dir_fd=directory_fd,
    )
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(content)


def redact(text: str) -> str:
    if re.search(r"-----BEGIN (?:[A-Z0-9 ]+ )?PRIVATE KEY-----", text):
        fail("source contains private-key material and cannot be collected")
    text = JSON_SECRET_RE.sub(r"\1[REDACTED]\2", text)
    text = SECRET_RE.sub(r"\1[REDACTED]", text)
    text = AUTH_RE.sub(r"\1 [REDACTED]", text)
    return USERINFO_RE.sub(r"\1[REDACTED]@", text)


def main() -> None:
    if len(sys.argv) != 4:
        fail("usage: collect.py INPUTS.env FILES.tsv NEW_EVIDENCE_DIRECTORY")
    inputs_path, allowlist_path, destination = map(Path, sys.argv[1:])
    if not DESTINATION_RE.fullmatch(destination.name):
        fail("destination name must be UTC timestamp plus path-a, path-b, path-c, or common")
    inputs = parse_inputs(inputs_path)
    entries = parse_allowlist(allowlist_path)
    _, parent_fd = verify_parent(destination)
    created_files: list[tuple[int, str]] = []
    destination_created = False
    destination_fd = -1
    files_fd = -1
    try:
        os.mkdir(destination.name, 0o700, dir_fd=parent_fd)
        destination_created = True
        destination_fd = os.open(
            destination.name, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW, dir_fd=parent_fd
        )
        os.mkdir("files", 0o700, dir_fd=destination_fd)
        files_fd = os.open("files", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW, dir_fd=destination_fd)
        index_lines = ["kind\tsource\tcollected\n"]
        checksums: dict[str, str] = {}
        for count, (kind, source) in enumerate(entries, 1):
            content = redact(read_text_once(source)).encode("utf-8")
            name = f"{count:03d}-{kind}.txt"
            write_file(files_fd, name, content)
            created_files.append((files_fd, name))
            relative = f"files/{name}"
            index_lines.append(f"{kind}\t{source}\t{relative}\n")
            checksums[relative] = hashlib.sha256(content).hexdigest()

        normalized = "".join(f"{key}={inputs[key]}\n" for key in ALLOWED_INPUT_KEYS).encode()
        index = "".join(index_lines).encode()
        write_file(destination_fd, "lab-inputs.env", normalized)
        created_files.append((destination_fd, "lab-inputs.env"))
        checksums["lab-inputs.env"] = hashlib.sha256(normalized).hexdigest()
        write_file(destination_fd, "source-index.tsv", index)
        created_files.append((destination_fd, "source-index.tsv"))
        checksums["source-index.tsv"] = hashlib.sha256(index).hexdigest()
        checksum_data = "".join(f"{digest}  ./{name}\n" for name, digest in sorted(checksums.items())).encode()
        write_file(destination_fd, "SHA256SUMS", checksum_data)
        created_files.append((destination_fd, "SHA256SUMS"))
    except BaseException:
        for directory_fd, name in reversed(created_files):
            try:
                os.unlink(name, dir_fd=directory_fd)
            except OSError:
                pass
        if files_fd >= 0:
            try:
                os.rmdir("files", dir_fd=destination_fd)
            except OSError:
                pass
        if destination_fd >= 0:
            os.close(destination_fd)
            destination_fd = -1
        if destination_created:
            try:
                os.rmdir(destination.name, dir_fd=parent_fd)
            except OSError:
                pass
        raise
    finally:
        if files_fd >= 0:
            os.close(files_fd)
        if destination_fd >= 0:
            os.close(destination_fd)
        os.close(parent_fd)
    print(f"[RESULT] collected and redacted {len(entries)} file(s) into {destination}")
    print("[ACTION] manually inspect every output before publication")


if __name__ == "__main__":
    main()
