import os
import subprocess
import re

env = os.environ.copy()
env.update({
    'CMAKE': r'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe',
    'CMAKE_GENERATOR': 'Visual Studio 17 2022',
    'LIBCLANG_PATH': r'D:\FinText-Alpha-Vectorizer\venv\Lib\site-packages\clang\native',
    'WHISPER_MOCK_FALLBACK': '1',
    'QUESTDB_MOCK_FALLBACK': '1',
    'NATS_MOCK_MODE': '1',
    'POLYGON_MOCK_MODE': '1',
    'DLQ_MOCK_MODE': '1',
    'ANOMALY_MOCK_MODE': '1'
})

packages = [
    ('html_sanitizer', 'fintext_html_sanitizer'),
    ('ticker_extractor', 'fintext_ticker_extractor'),
    ('spam_detector', 'fintext_spam_detector'),
    ('event_classifier', 'fintext_event_classifier'),
    ('sidecar', 'fintext_rust_sidecar'),
    ('ingestion_engine', 'fintext_ingestion_engine'),
    ('spillover_engine', 'fintext_spillover_engine'),
    ('api_server', 'fintext_api_server'),
    ('dead_letter_worker', 'fintext_dead_letter_worker'),
    ('observability_anomaly', 'fintext_observability_anomaly')
]

print("=== Comprehensive Rust Workspace Verification (10 Crates) ===\n")
header = f"{'#':<3} | {'Directory':<24} | {'Package Name':<32} | {'Compilation':<12} | {'Tests Passed':<12} | {'Tests Failed':<12}"
print(header)
print("-" * len(header))

total_passed = 0
total_failed = 0

for i, (d, pkg) in enumerate(packages, 1):
    res = subprocess.run(
        ['cargo', 'test', '--package', pkg, '--manifest-path', 'rust/Cargo.toml'],
        capture_output=True,
        text=True,
        env=env
    )
    output = res.stdout + "\n" + res.stderr
    
    matches = re.findall(r'test result:\s*ok\.\s*(\d+)\s*passed;\s*(\d+)\s*failed', output)
    p_cnt = sum(int(m[0]) for m in matches)
    f_cnt = sum(int(m[1]) for m in matches)
    
    comp_status = "OK (Exit 0)" if res.returncode == 0 else f"FAIL ({res.returncode})"
    total_passed += p_cnt
    total_failed += f_cnt
    
    print(f"{i:<3} | {d:<24} | {pkg:<32} | {comp_status:<12} | {p_cnt:<12} | {f_cnt:<12}")

print("-" * len(header))
print(f"Total Crates: 10 | Total Unit Tests Passed: {total_passed} | Total Unit Tests Failed: {total_failed}\n")
