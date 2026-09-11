#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #234: FIX Protocol Bridge & Execution Engine
===============================================================================
Verifies:
  1.  Unauthenticated POST /fix/order returns 401 Unauthorized
  2.  Unauthenticated GET /fix/orders returns 401 Unauthorized
  3.  Unauthenticated POST /fix/cancel returns 401 Unauthorized
  4.  Missing required tags (e.g. tag 55 missing) returns 400 Bad Request with FIX Reject (35=3)
  5.  Invalid Side / OrdType returns 400 Bad Request with FIX Reject (35=3)
  6.  Submit Buy Market Order (35=D, 54=1, 40=1) returns 200 OK with Fill Execution Report (35=8, 150=2, 39=2)
  7.  Submit Sell Market Order (35=D, 54=2, 40=1) returns 200 OK with Fill Execution Report
  8.  Submit Limit Buy Order below mock price returns 200 OK with Open Status (35=8, 150=0, 39=0)
  9.  Submit Limit Sell Order above mock price returns 200 OK with Open Status (35=8, 150=0, 39=0)
  10. Submit Marketable Limit Buy Order (Price >= mock price) fills immediately (35=8, 150=2, 39=2)
  11. List FIX orders (GET /fix/orders) returns 200 OK with paginated order list
  12. Filter FIX orders by status query parameter ('open', 'filled', 'cancelled')
  13. Cancel open order (POST /fix/cancel with 35=F) returns 200 OK with Cancelled Report (150=4, 39=4)
  14. Cancel already filled or cancelled order returns 400 Bad Request
  15. Cancel non-existent order (unknown 41=OrigClOrdID) returns 404 Not Found
  16. Python SDK Sync Client integration verification (submit_fix_order, list_fix_orders, cancel_fix_order)
  17. Python SDK Async Client integration verification (submit_fix_order, list_fix_orders, cancel_fix_order)
  18. Audit log recording (fix.order_submitted, fix.order_filled, fix.order_cancelled)
  19. Enterprise Role Gating: Non-enterprise tokens (e.g. institutional/retail) receive 403 Forbidden
  20. Feature Flag Gating: Server with ENABLE_FIX_BRIDGE=0 returns 404 Not Found for FIX endpoints
===============================================================================
"""

import asyncio
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SDK_PATH = PROJECT_ROOT / "python_sdk" / "src"
if str(SDK_PATH) not in sys.path:
    sys.path.insert(0, str(SDK_PATH))

PORT = 8133
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 20


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  \u2705 Phase {phase:2d} \u2502 {name}")
    else:
        failed += 1
        msg = f"  \u274c Phase {phase:2d} \u2502 {name}"
        if detail:
            msg += f" \u2014 {detail}"
        print(msg)


class ServerContext:
    def __init__(self, port: int = PORT, enable_fix_bridge: str = "1"):
        self.port = port
        self.base_url = f"http://127.0.0.1:{port}"
        self.enable_fix_bridge = enable_fix_bridge
        self.process = None

    def __enter__(self):
        print(f"[STARTING] Spawning FinText API Server on port {self.port} (ENABLE_FIX_BRIDGE={self.enable_fix_bridge})...")
        env = os.environ.copy()
        env["PORT"] = str(self.port)
        env["HOST"] = "127.0.0.1"
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = "super_secret_test_jwt_key_32_bytes_len!!"
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"
        env["CHAT_ALERTS_MOCK"] = "1"
        env["ENABLE_FIX_BRIDGE"] = self.enable_fix_bridge

        self.process = subprocess.Popen(
            [str(SERVER_EXE)],
            env=env,
            cwd=str(PROJECT_ROOT),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        start_t = time.time()
        ready = False
        while time.time() - start_t < 25:
            try:
                r = httpx.get(f"{self.base_url}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)

        if not ready:
            if self.process.poll() is not None:
                raise RuntimeError("FinText API server process exited prematurely")
            raise RuntimeError("FinText API server failed to start within 25 seconds")

        print(f"[READY] FinText API Server is responding on {self.base_url}.\n")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print(f"[STOPPING] Terminating FinText API Server (PID: {self.process.pid})...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_jwt_token(client: httpx.Client, user_id: str = "trader_fix_01", role: str = "enterprise") -> str:
    r = client.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id, "expires_in_seconds": 3600, "role": role},
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    if r.status_code != 200:
        raise RuntimeError(f"Failed to issue JWT token: {r.status_code} - {r.text}")
    return r.json()["token"]


def main():
    global passed, failed
    print("=" * 80)
    print("  FinText-Alpha-Vectorizer — FIX Protocol Bridge Certification (#234)")
    print("=" * 80)

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)
        jwt_token = get_jwt_token(client, user_id="trader_fix_01")
        auth_headers = {"Authorization": f"Bearer {jwt_token}", "Content-Type": "application/json"}

        # -----------------------------------------------------------------
        # Phase 1: Unauthenticated POST /fix/order returns 401
        # -----------------------------------------------------------------
        r1 = client.post("/fix/order", json={"fix_message": "8=FIX.4.4|35=D|11=O1|55=AAPL|54=1|38=100|40=1|10=000|"})
        report(1, "Unauthenticated POST /fix/order returns 401", r1.status_code == 401)

        # -----------------------------------------------------------------
        # Phase 2: Unauthenticated GET /fix/orders returns 401
        # -----------------------------------------------------------------
        r2 = client.get("/fix/orders")
        report(2, "Unauthenticated GET /fix/orders returns 401", r2.status_code == 401)

        # -----------------------------------------------------------------
        # Phase 3: Unauthenticated POST /fix/cancel returns 401
        # -----------------------------------------------------------------
        r3 = client.post("/fix/cancel", json={"fix_message": "8=FIX.4.4|35=F|11=C1|41=O1|10=000|"})
        report(3, "Unauthenticated POST /fix/cancel returns 401", r3.status_code == 401)

        # -----------------------------------------------------------------
        # Phase 4: Missing required tags (tag 55 symbol missing) returns 400 with FIX Reject (35=3)
        # -----------------------------------------------------------------
        r4 = client.post("/fix/order", json={"fix_message": "8=FIX.4.4|35=D|11=MISS-SYM|54=1|38=100|40=1|10=000|"}, headers=auth_headers)
        data4 = r4.json() if r4.status_code == 400 else {}
        ok4 = r4.status_code == 400 and ("35=3" in data4.get("fix_message", "") or "Reject" in data4.get("message", ""))
        report(4, "Missing required tags returns 400 FIX Reject (35=3)", ok4, f"status: {r4.status_code}, body: {r4.text}")

        # -----------------------------------------------------------------
        # Phase 5: Invalid Side or OrdType returns 400 with FIX Reject
        # -----------------------------------------------------------------
        r5 = client.post("/fix/order", json={"fix_message": "8=FIX.4.4|35=D|11=INV-SIDE|55=AAPL|54=9|38=100|40=1|10=000|"}, headers=auth_headers)
        data5 = r5.json() if r5.status_code == 400 else {}
        ok5 = r5.status_code == 400 and ("35=3" in data5.get("fix_message", "") or "Reject" in data5.get("message", ""))
        report(5, "Invalid Side (tag 54=9) returns 400 FIX Reject", ok5, f"status: {r5.status_code}")

        # -----------------------------------------------------------------
        # Phase 6: Submit Buy Market Order (35=D, 54=1, 40=1) returns 200 OK with Fill Execution Report
        # -----------------------------------------------------------------
        cl_ord_mkt_buy = f"CL-MKT-BUY-{uuid.uuid4().hex[:6]}"
        fix_mkt_buy = f"8=FIX.4.4|9=120|35=D|11={cl_ord_mkt_buy}|55=AAPL|54=1|38=100|40=1|10=000|"
        r6 = client.post("/fix/order", json={"fix_message": fix_mkt_buy}, headers=auth_headers)
        data6 = r6.json() if r6.status_code == 200 else {}
        ok6 = (
            r6.status_code == 200
            and data6.get("status") == "filled"
            and data6.get("exec_type") == "2"
            and data6.get("filled_qty") == 100.0
            and data6.get("avg_price") is not None
            and "35=8" in data6.get("fix_message", "")
            and "150=2" in data6.get("fix_message", "")
            and "39=2" in data6.get("fix_message", "")
        )
        report(6, "Submit Buy Market Order returns 200 filled execution report", ok6, f"status: {r6.status_code}, data: {data6}")

        # -----------------------------------------------------------------
        # Phase 7: Submit Sell Market Order (35=D, 54=2, 40=1) returns 200 OK with Fill
        # -----------------------------------------------------------------
        cl_ord_mkt_sell = f"CL-MKT-SELL-{uuid.uuid4().hex[:6]}"
        fix_mkt_sell = f"8=FIX.4.4|9=120|35=D|11={cl_ord_mkt_sell}|55=NVDA|54=2|38=250|40=1|10=000|"
        r7 = client.post("/fix/order", json={"fix_message": fix_mkt_sell}, headers=auth_headers)
        data7 = r7.json() if r7.status_code == 200 else {}
        ok7 = (
            r7.status_code == 200
            and data7.get("status") == "filled"
            and data7.get("exec_type") == "2"
            and data7.get("filled_qty") == 250.0
            and data7.get("symbol") == "NVDA"
            and data7.get("side") == "2"
        )
        report(7, "Submit Sell Market Order returns 200 filled execution report", ok7, f"status: {r7.status_code}, data: {data7}")

        # -----------------------------------------------------------------
        # Phase 8: Submit Limit Buy Order below mock price returns 200 OK with Open Status
        # -----------------------------------------------------------------
        cl_ord_lim_open = f"CL-LIM-OPEN-{uuid.uuid4().hex[:6]}"
        # Mock price for MSFT is ~75.00. Set Buy Limit = 10.00 (< mock price)
        fix_lim_open = f"8=FIX.4.4|9=130|35=D|11={cl_ord_lim_open}|55=MSFT|54=1|38=50|40=2|44=10.00|10=000|"
        r8 = client.post("/fix/order", json={"fix_message": fix_lim_open}, headers=auth_headers)
        data8 = r8.json() if r8.status_code == 200 else {}
        ok8 = (
            r8.status_code == 200
            and data8.get("status") == "open"
            and data8.get("exec_type") == "0"
            and data8.get("filled_qty") == 0.0
            and data8.get("avg_price") is None
            and "150=0" in data8.get("fix_message", "")
            and "39=0" in data8.get("fix_message", "")
        )
        report(8, "Submit Limit Buy below mock price returns 200 open status", ok8, f"status: {r8.status_code}, data: {data8}")

        # -----------------------------------------------------------------
        # Phase 9: Submit Limit Sell Order above mock price returns 200 OK with Open Status
        # -----------------------------------------------------------------
        cl_ord_lim_sell_open = f"CL-LIM-SELL-OPEN-{uuid.uuid4().hex[:6]}"
        # Sell Limit = 200.00 (> mock price)
        fix_lim_sell_open = f"8=FIX.4.4|9=130|35=D|11={cl_ord_lim_sell_open}|55=GOOGL|54=2|38=80|40=2|44=200.00|10=000|"
        r9 = client.post("/fix/order", json={"fix_message": fix_lim_sell_open}, headers=auth_headers)
        data9 = r9.json() if r9.status_code == 200 else {}
        ok9 = (
            r9.status_code == 200
            and data9.get("status") == "open"
            and data9.get("exec_type") == "0"
            and data9.get("filled_qty") == 0.0
        )
        report(9, "Submit Limit Sell above mock price returns 200 open status", ok9, f"status: {r9.status_code}, data: {data9}")

        # -----------------------------------------------------------------
        # Phase 10: Submit Marketable Limit Buy Order fills immediately
        # -----------------------------------------------------------------
        cl_ord_lim_fill = f"CL-LIM-FILL-{uuid.uuid4().hex[:6]}"
        # Buy Limit = 150.00 (>= mock price)
        fix_lim_fill = f"8=FIX.4.4|9=130|35=D|11={cl_ord_lim_fill}|55=TSLA|54=1|38=60|40=2|44=150.00|10=000|"
        r10 = client.post("/fix/order", json={"fix_message": fix_lim_fill}, headers=auth_headers)
        data10 = r10.json() if r10.status_code == 200 else {}
        ok10 = (
            r10.status_code == 200
            and data10.get("status") == "filled"
            and data10.get("exec_type") == "2"
            and data10.get("filled_qty") == 60.0
            and data10.get("avg_price") is not None
        )
        report(10, "Marketable Limit Buy (Price >= mock price) fills immediately", ok10, f"status: {r10.status_code}, data: {data10}")

        # -----------------------------------------------------------------
        # Phase 11: List FIX orders (GET /fix/orders) returns 200 OK with paginated list
        # -----------------------------------------------------------------
        r11 = client.get("/fix/orders?limit=10&offset=0", headers=auth_headers)
        data11 = r11.json() if r11.status_code == 200 else {}
        orders11 = data11.get("orders", [])
        ok11 = (
            r11.status_code == 200
            and isinstance(orders11, list)
            and len(orders11) >= 5
            and data11.get("total", 0) >= 5
            and "symbol" in orders11[0]
            and "cl_ord_id" in orders11[0]
        )
        report(11, "List FIX orders returns 200 with order items and total", ok11, f"status: {r11.status_code}, total: {data11.get('total')}")

        # -----------------------------------------------------------------
        # Phase 12: Filter FIX orders by status ('open', 'filled')
        # -----------------------------------------------------------------
        r12_open = client.get("/fix/orders?status=open", headers=auth_headers)
        data12_open = r12_open.json() if r12_open.status_code == 200 else {}
        r12_filled = client.get("/fix/orders?status=filled", headers=auth_headers)
        data12_filled = r12_filled.json() if r12_filled.status_code == 200 else {}
        ok12 = (
            r12_open.status_code == 200
            and all(o["status"] == "open" for o in data12_open.get("orders", []))
            and r12_filled.status_code == 200
            and all(o["status"] == "filled" for o in data12_filled.get("orders", []))
            and len(data12_open.get("orders", [])) >= 2
            and len(data12_filled.get("orders", [])) >= 3
        )
        report(12, "Filter FIX orders by status ('open', 'filled')", ok12, f"open: {len(data12_open.get('orders', []))}, filled: {len(data12_filled.get('orders', []))}")

        # -----------------------------------------------------------------
        # Phase 13: Cancel open order (POST /fix/cancel) returns 200 Cancelled Report
        # -----------------------------------------------------------------
        cl_cancel_01 = f"CANC-{uuid.uuid4().hex[:6]}"
        fix_cancel = f"8=FIX.4.4|9=120|35=F|11={cl_cancel_01}|41={cl_ord_lim_open}|55=MSFT|54=1|38=50|10=000|"
        r13 = client.post("/fix/cancel", json={"fix_message": fix_cancel}, headers=auth_headers)
        data13 = r13.json() if r13.status_code == 200 else {}
        ok13 = (
            r13.status_code == 200
            and data13.get("status") == "cancelled"
            and data13.get("exec_type") == "4"
            and "150=4" in data13.get("fix_message", "")
            and "39=4" in data13.get("fix_message", "")
        )
        report(13, "Cancel open order returns 200 with cancelled execution report", ok13, f"status: {r13.status_code}, data: {data13}")

        # -----------------------------------------------------------------
        # Phase 14: Cancel already cancelled / filled order returns 400 Bad Request
        # -----------------------------------------------------------------
        # Try cancelling the same order again
        r14 = client.post("/fix/cancel", json={"fix_message": fix_cancel}, headers=auth_headers)
        report(14, "Cancel already cancelled order returns 400 Bad Request", r14.status_code == 400, f"status: {r14.status_code}")

        # -----------------------------------------------------------------
        # Phase 15: Cancel non-existent order returns 404 Not Found
        # -----------------------------------------------------------------
        fix_cancel_non_existent = "8=FIX.4.4|9=120|35=F|11=CANC-UNKNOWN|41=NON-EXISTENT-CLORDID|55=MSFT|54=1|38=50|10=000|"
        r15 = client.post("/fix/cancel", json={"fix_message": fix_cancel_non_existent}, headers=auth_headers)
        report(15, "Cancel non-existent OrigClOrdID returns 404 Not Found", r15.status_code == 404, f"status: {r15.status_code}")

        # -----------------------------------------------------------------
        # Phase 16: Python SDK Sync Client Integration
        # -----------------------------------------------------------------
        from fintext import FinTextClient, FIXOrderRequest, FIXOrderResponse, FIXOrdersListResponse

        sync_sdk_client = FinTextClient(base_url=BASE_URL, api_token=jwt_token)
        sdk_cl_ord = f"SDK-SYNC-{uuid.uuid4().hex[:6]}"
        sdk_fix_msg = f"8=FIX.4.4|9=120|35=D|11={sdk_cl_ord}|55=AMZN|54=1|38=75|40=1|10=000|"

        sdk_order_resp = sync_sdk_client.submit_fix_order(sdk_fix_msg)
        sdk_list_resp = sync_sdk_client.list_fix_orders(status="filled", limit=5)

        sdk_lim_cl_ord = f"SDK-SYNC-LIM-{uuid.uuid4().hex[:6]}"
        sdk_lim_msg = f"8=FIX.4.4|9=130|35=D|11={sdk_lim_cl_ord}|55=AMZN|54=1|38=20|40=2|44=10.00|10=000|"
        sdk_lim_resp = sync_sdk_client.submit_fix_order(FIXOrderRequest(fix_message=sdk_lim_msg))

        sdk_cancel_msg = f"8=FIX.4.4|9=120|35=F|11=SDK-CANC-01|41={sdk_lim_cl_ord}|55=AMZN|54=1|38=20|10=000|"
        sdk_cancel_resp = sync_sdk_client.cancel_fix_order(sdk_cancel_msg)

        ok16 = (
            isinstance(sdk_order_resp, FIXOrderResponse)
            and sdk_order_resp.status == "filled"
            and sdk_order_resp.symbol == "AMZN"
            and isinstance(sdk_list_resp, FIXOrdersListResponse)
            and len(sdk_list_resp.orders) >= 1
            and sdk_lim_resp.status == "open"
            and sdk_cancel_resp.status == "cancelled"
        )
        sync_sdk_client.close()
        report(16, "Python SDK Sync Client submit, list, and cancel methods", ok16)

        # -----------------------------------------------------------------
        # Phase 17: Python SDK Async Client Integration
        # -----------------------------------------------------------------
        from fintext import FinTextAsyncClient

        async def run_async_sdk_tests():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=jwt_token)
            a_cl_ord = f"SDK-ASYNC-{uuid.uuid4().hex[:6]}"
            a_fix_msg = f"8=FIX.4.4|9=120|35=D|11={a_cl_ord}|55=META|54=2|38=120|40=1|10=000|"

            a_order_resp = await async_client.submit_fix_order(a_fix_msg)
            a_list_resp = await async_client.list_fix_orders(limit=5)

            a_lim_ord = f"SDK-ASYNC-LIM-{uuid.uuid4().hex[:6]}"
            a_lim_msg = f"8=FIX.4.4|9=130|35=D|11={a_lim_ord}|55=META|54=2|38=40|40=2|44=300.00|10=000|"
            a_lim_resp = await async_client.submit_fix_order(a_lim_msg)

            a_canc_msg = f"8=FIX.4.4|9=120|35=F|11=SDK-ASYNC-CANC|41={a_lim_ord}|55=META|54=2|38=40|10=000|"
            a_canc_resp = await async_client.cancel_fix_order(a_canc_msg)

            await async_client.close()
            return (
                isinstance(a_order_resp, FIXOrderResponse)
                and a_order_resp.status == "filled"
                and isinstance(a_list_resp, FIXOrdersListResponse)
                and a_lim_resp.status == "open"
                and a_canc_resp.status == "cancelled"
            )

        ok17 = asyncio.run(run_async_sdk_tests())
        report(17, "Python SDK Async Client submit, list, and cancel methods", ok17)

        # -----------------------------------------------------------------
        # Phase 18: Audit Log Recording
        # -----------------------------------------------------------------
        r18 = client.get("/audit/logs?limit=50", headers=auth_headers)
        data18 = r18.json() if r18.status_code == 200 else {}
        actions_logged = [log.get("action") for log in data18.get("logs", [])]
        ok18 = (
            r18.status_code == 200
            and "fix.order_submitted" in actions_logged
            and "fix.order_filled" in actions_logged
            and "fix.order_cancelled" in actions_logged
        )
        report(18, "Audit log recording of FIX actions (submitted, filled, cancelled)", ok18, f"actions: {set(actions_logged)}")

        # -----------------------------------------------------------------
        # Phase 19: Enterprise Role Gating (403 Forbidden for Non-Enterprise)
        # -----------------------------------------------------------------
        non_ent_token = get_jwt_token(client, user_id="retail_user_01", role="institutional")
        non_ent_headers = {"Authorization": f"Bearer {non_ent_token}", "Content-Type": "application/json"}

        r19_order = client.post("/fix/order", json={"fix_message": "8=FIX.4.4|35=D|11=O1|55=AAPL|54=1|38=100|40=1|10=000|"}, headers=non_ent_headers)
        r19_list = client.get("/fix/orders", headers=non_ent_headers)
        r19_cancel = client.post("/fix/cancel", json={"fix_message": "8=FIX.4.4|35=F|11=C1|41=O1|10=000|"}, headers=non_ent_headers)

        ok19 = (
            r19_order.status_code == 403
            and r19_list.status_code == 403
            and r19_cancel.status_code == 403
            and "enterprise-only" in r19_order.json().get("message", "").lower()
        )
        report(19, "Enterprise Role Gating: Non-enterprise tokens receive 403 Forbidden", ok19)

    # -----------------------------------------------------------------
    # Phase 20: Feature Flag Gating (ENABLE_FIX_BRIDGE=0 returns 404)
    # -----------------------------------------------------------------
    port_disabled = 8134
    with ServerContext(port=port_disabled, enable_fix_bridge="0"):
        disabled_client = httpx.Client(base_url=f"http://127.0.0.1:{port_disabled}", timeout=10.0)
        # Issue token on disabled server
        r_tok = disabled_client.post(
            "/auth/token",
            json={"user_id": "ent_user_01", "expires_in_seconds": 3600, "role": "enterprise"},
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        ent_tok_dis = r_tok.json()["token"]
        ent_headers_dis = {"Authorization": f"Bearer {ent_tok_dis}", "Content-Type": "application/json"}

        r20_order = disabled_client.post("/fix/order", json={"fix_message": "8=FIX.4.4|35=D|11=O1|55=AAPL|54=1|38=100|40=1|10=000|"}, headers=ent_headers_dis)
        r20_list = disabled_client.get("/fix/orders", headers=ent_headers_dis)
        r20_cancel = disabled_client.post("/fix/cancel", json={"fix_message": "8=FIX.4.4|35=F|11=C1|41=O1|10=000|"}, headers=ent_headers_dis)

        ok20 = (
            r20_order.status_code == 404
            and r20_list.status_code == 404
            and r20_cancel.status_code == 404
        )
        report(20, "Feature Flag Gating: ENABLE_FIX_BRIDGE=0 returns 404 for FIX endpoints", ok20)

    print("\n" + "=" * 80)
    print(f"  Summary: {passed}/{total} Passed | {failed} Failed")
    print("=" * 80)

    if passed == total:
        print("  \u2705 ALL FIX PROTOCOL BRIDGE TESTS PASSED CLEANLY!\n")
        return 0
    else:
        print("  \u274c SOME FIX PROTOCOL BRIDGE TESTS FAILED!\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())
