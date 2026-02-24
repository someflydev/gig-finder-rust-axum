#!/usr/bin/env python3
import json
import sys
from pathlib import Path

try:
    import yaml  # type: ignore
except ImportError:
    yaml = None

ROOT = Path(__file__).resolve().parents[1]
SOURCES_YAML = ROOT / "sources.yaml"
ADAPTER_LIB = ROOT / "crates" / "rhof-adapters" / "src" / "lib.rs"
ADAPTER_TESTS_DIR = ROOT / "crates" / "rhof-adapters" / "tests"


def load_sources():
    text = SOURCES_YAML.read_text()
    if yaml is not None:
        data = yaml.safe_load(text)
        if not isinstance(data, dict) or "sources" not in data or not isinstance(data["sources"], list):
            raise SystemExit("ERROR: sources.yaml must contain top-level 'sources:' list")
        return data["sources"]

    # Fallback parser for the repo's simple fixed schema (top-level `sources:` list).
    lines = text.splitlines()
    if not any(line.strip() == "sources:" for line in lines):
        raise SystemExit("ERROR: sources.yaml must contain top-level 'sources:' list")
    sources = []
    current = None
    for raw in lines:
        line = raw.rstrip()
        stripped = line.strip()
        if stripped.startswith("- source_id:"):
            if current:
                sources.append(current)
            current = {"source_id": stripped.split(":", 1)[1].strip()}
        elif current and ":" in stripped and not stripped.startswith("- "):
            key, value = stripped.split(":", 1)
            current[key.strip()] = value.strip()
    if current:
        sources.append(current)
    return sources


def has_test_reference(source_id: str, adapter_lib_text: str) -> bool:
    if source_id in adapter_lib_text:
        return True
    if ADAPTER_TESTS_DIR.exists():
        for path in ADAPTER_TESTS_DIR.glob("*.rs"):
            if source_id in path.read_text():
                return True
    return False


def check_fixture_bundle(source_id: str):
    bundle = ROOT / "fixtures" / source_id / "sample" / "bundle.json"
    snapshot = ROOT / "fixtures" / source_id / "sample" / "snapshot.json"
    errors = []
    if not bundle.exists():
        errors.append(f"missing fixture bundle: {bundle}")
        return errors
    if not snapshot.exists():
        errors.append(f"missing snapshot file: {snapshot}")

    try:
        payload = json.loads(bundle.read_text())
    except Exception as exc:
        errors.append(f"invalid JSON in {bundle}: {exc}")
        return errors

    if payload.get("source_id") != source_id:
        errors.append(f"bundle source_id mismatch in {bundle}")
    if not payload.get("extractor_version"):
        errors.append(f"missing extractor_version in {bundle}")
    if "crawlability" not in payload:
        errors.append(f"missing crawlability in {bundle}")
    if "raw_artifact" not in payload:
        errors.append(f"missing raw_artifact block in {bundle}")
    parsed_records = payload.get("parsed_records")
    if not isinstance(parsed_records, list):
        errors.append(f"parsed_records must be a list in {bundle}")
    elif len(parsed_records) == 0:
        errors.append(f"parsed_records must contain at least one record in {bundle}")
    coverage = payload.get("evidence_coverage_percent")
    if not isinstance(coverage, (int, float)):
        errors.append(f"missing or invalid evidence_coverage_percent in {bundle}")
    elif coverage < 90:
        errors.append(f"evidence_coverage_percent < 90 in {bundle} (got {coverage})")

    if snapshot.exists():
        try:
            snap_payload = json.loads(snapshot.read_text())
        except Exception as exc:
            errors.append(f"invalid JSON in {snapshot}: {exc}")
        else:
            if not isinstance(snap_payload, list):
                errors.append(f"snapshot must be a JSON array in {snapshot}")
            elif len(snap_payload) == 0:
                errors.append(f"snapshot must contain at least one parsed record in {snapshot}")
    return errors


def main():
    sources = load_sources()
    adapter_lib_text = ADAPTER_LIB.read_text() if ADAPTER_LIB.exists() else ""
    errors = []

    for src in sources:
        source_id = src.get("source_id")
        if not source_id:
            errors.append("sources.yaml entry missing source_id")
            continue
        errors.extend(check_fixture_bundle(source_id))
        if not has_test_reference(source_id, adapter_lib_text):
            errors.append(f"missing parse test reference for source_id={source_id}")

    if errors:
        print("Adapter contract checks failed:", file=sys.stderr)
        for err in errors:
            print(f"- {err}", file=sys.stderr)
        return 1

    print(f"Adapter contract checks passed for {len(sources)} sources")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
