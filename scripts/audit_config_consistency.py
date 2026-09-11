import os
import sys
import re
import json
import yaml
from pathlib import Path

root = Path('.')
sys.stdout.reconfigure(encoding='utf-8')

def run_audit():
    print("=" * 80)
    print(" FinText Alpha Vectorizer — Configuration & Environment Consistency Audit")
    print("=" * 80)

    # 1. Extract env vars in Rust source files
    rust_env_vars = set()
    rust_var_locations = {}
    pattern = re.compile(r'(?:std::)?env::var(?:_os)?\s*\(\s*"([A-Za-z0-9_]+)"\s*\)')

    for rs_file in (root / 'rust').glob('**/*.rs'):
        content = rs_file.read_text(encoding='utf-8', errors='ignore')
        for match in pattern.finditer(content):
            var_name = match.group(1)
            rust_env_vars.add(var_name)
            rust_var_locations.setdefault(var_name, []).append(str(rs_file.relative_to(root)).replace('\\', '/'))

    print(f"\n[1] Rust Codebase Environment Variables ({len(rust_env_vars)} Unique Vars):")
    for var in sorted(rust_env_vars):
        locs = ', '.join(sorted(set(rust_var_locations[var])))
        print(f"  - {var:<30} (used in: {locs})")

    # 2. Extract env vars from .env and .env.example
    def parse_env_file(filepath):
        vars_found = set()
        if filepath.exists():
            for line in filepath.read_text(encoding='utf-8-sig', errors='ignore').splitlines():
                line = line.strip()
                if line and not line.startswith('#') and '=' in line:
                    key = line.split('=', 1)[0].strip()
                    vars_found.add(key)
        return vars_found

    dotenv_vars = parse_env_file(root / '.env')
    dotenv_example_vars = parse_env_file(root / '.env.example')

    print(f"\n[2] .env Variables ({len(dotenv_vars)}):")
    print("  " + ", ".join(sorted(dotenv_vars)))
    print(f"\n[3] .env.example Variables ({len(dotenv_example_vars)}):")
    print("  " + ", ".join(sorted(dotenv_example_vars)))

    # 3. Extract env vars and ports from docker-compose.yml
    dc_vars = set()
    dc_ports = []
    dc_services = {}
    if (root / 'docker-compose.yml').exists():
        dc_data = yaml.safe_load((root / 'docker-compose.yml').read_text(encoding='utf-8'))
        for s_name, s_conf in dc_data.get('services', {}).items():
            dc_services[s_name] = s_conf
            for env_entry in s_conf.get('environment', []):
                if isinstance(env_entry, str):
                    var = env_entry.split('=', 1)[0].strip()
                    dc_vars.add(var)
                elif isinstance(env_entry, dict):
                    for k in env_entry.keys():
                        dc_vars.add(k)
            for p in s_conf.get('ports', []):
                dc_ports.append((s_name, p))

    print(f"\n[4] Docker Compose Services ({len(dc_services)}) & Variables ({len(dc_vars)}):")
    print("  " + ", ".join(sorted(dc_vars)))

    # 4. Extract env vars and ports from Dockerfile
    dockerfile_vars = set()
    if (root / 'Dockerfile').exists():
        df_content = (root / 'Dockerfile').read_text(encoding='utf-8')
        for line in df_content.splitlines():
            line = line.strip()
            if line.startswith('ENV '):
                # parse ENV VAR="val" \ VAR2="val2"
                tokens = re.findall(r'([A-Za-z0-9_]+)=', line)
                for t in tokens:
                    dockerfile_vars.add(t)

    print(f"\n[5] Dockerfile Defined Variables ({len(dockerfile_vars)}):")
    print("  " + ", ".join(sorted(dockerfile_vars)))

    # 5. Extract env vars and secrets from k8s manifests
    k8s_vars = set()
    k8s_secrets = {}
    k8s_secret_refs = []
    k8s_ports = []
    k8s_workloads = {}

    for k8s_file in (root / 'k8s').glob('**/*.yaml'):
        content = k8s_file.read_text(encoding='utf-8')
        for doc in yaml.safe_load_all(content):
            if not isinstance(doc, dict):
                continue
            kind = doc.get('kind', '')
            name = doc.get('metadata', {}).get('name', '')
            if kind == 'Secret':
                string_data = doc.get('stringData', {})
                k8s_secrets[name] = list(string_data.keys())
            
            # Check containers env & ports
            spec = doc.get('spec', {})
            if 'template' in spec:
                spec = spec['template'].get('spec', {})
            containers = spec.get('containers', []) + spec.get('initContainers', [])
            if containers:
                k8s_workloads[f"{kind}/{name}"] = str(k8s_file.relative_to(root)).replace('\\', '/')
            for c in containers:
                for p in c.get('ports', []):
                    k8s_ports.append((name, p.get('name', ''), p.get('containerPort', '')))
                for env_item in c.get('env', []):
                    if 'name' in env_item:
                        k8s_vars.add(env_item['name'])
                    if 'valueFrom' in env_item:
                        sec_ref = env_item['valueFrom'].get('secretKeyRef', {})
                        if sec_ref:
                            k8s_secret_refs.append((
                                str(k8s_file.relative_to(root)).replace('\\', '/'),
                                name,
                                sec_ref.get('name'),
                                sec_ref.get('key')
                            ))

    print(f"\n[6] Kubernetes Manifest Variables ({len(k8s_vars)}) & Workloads ({len(k8s_workloads)}):")
    print("  " + ", ".join(sorted(k8s_vars)))

    # 6. Extract env vars from CI
    ci_vars = set()
    if (root / '.github/workflows/ci.yml').exists():
        ci_content = (root / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        ci_data = yaml.safe_load(ci_content)
        for k in ci_data.get('env', {}).keys():
            ci_vars.add(k)

    print(f"\n[7] CI Workflow Variables ({len(ci_vars)}):")
    print("  " + ", ".join(sorted(ci_vars)))

    # ── SECTION 1: Comparison & Consistency ──────────────────────────────────
    all_defined_vars = dotenv_vars | dotenv_example_vars | dc_vars | dockerfile_vars | k8s_vars | ci_vars

    missing_in_config = sorted(list(rust_env_vars - all_defined_vars))
    unused_in_rust = sorted(list(all_defined_vars - rust_env_vars))

    print("\n" + "=" * 80)
    print(" ANALYSIS SUMMARY")
    print("=" * 80)
    print(f"Total Unique Env Vars Referenced in Rust: {len(rust_env_vars)}")
    print(f"Total Unique Env Vars Defined Across Configs: {len(all_defined_vars)}")
    print(f"Variables Used in Rust but Missing in ALL Configs: {missing_in_config}")

    # ── SECTION 2: Port Collision Audit ──────────────────────────────────────
    print("\n[Port Mapping Audit - Docker Compose]:")
    host_ports = {}
    for svc, p_str in dc_ports:
        parts = str(p_str).split(':')
        host_p = parts[0]
        cont_p = parts[1] if len(parts) > 1 else parts[0]
        if host_p in host_ports:
            print(f"  [COLLISION ERROR] Port {host_p} mapped by multiple services: {host_ports[host_p]} and {svc}")
        else:
            host_ports[host_p] = (svc, cont_p)
            print(f"  [OK] Host Port {host_p:<6} -> Service: {svc:<25} (Container Port: {cont_p})")

    # ── SECTION 3: Kubernetes Secret References Validation ───────────────────
    print("\n[Kubernetes Secrets Validation]:")
    print(f"Defined Secrets: {list(k8s_secrets.keys())}")
    for kfile, wname, sname, skey in k8s_secret_refs:
        if sname not in k8s_secrets:
            print(f"  [ERROR] Workload '{wname}' in '{kfile}' references undefined Secret '{sname}'")
        elif skey not in k8s_secrets[sname]:
            print(f"  [ERROR] Workload '{wname}' references key '{skey}' not found in Secret '{sname}' ({k8s_secrets[sname]})")
        else:
            print(f"  [OK] Secret Reference valid: '{wname}' -> Secret '{sname}' [key: {skey}]")

    # ── SECTION 4: Docker Compose depends_on validation ──────────────────────
    print("\n[Docker Compose depends_on Validation]:")
    for s_name, s_conf in dc_services.items():
        deps = s_conf.get('depends_on', {})
        if isinstance(deps, list):
            dep_names = deps
        elif isinstance(deps, dict):
            dep_names = list(deps.keys())
        else:
            dep_names = []
        for d in dep_names:
            if d not in dc_services:
                print(f"  [ERROR] Service '{s_name}' depends on non-existent service '{d}'")
            else:
                print(f"  [OK] Dependency valid: {s_name} -> {d}")

if __name__ == '__main__':
    run_audit()
