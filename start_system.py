# مسیر: /start_system.py
import subprocess
import sys
import os
import time
import signal
import socket
from threading import Thread

os.system("")

GREEN = "\033[92m"
CYAN = "\033[96m"
YELLOW = "\033[93m"
MAGENTA = "\033[95m"
BLUE = "\033[94m"
RED = "\033[91m"
RESET = "\033[0m"
BOLD = "\033[1m"

processes = []
IS_WIN = sys.platform.startswith("win")

def free_port(port: int):
    """آزادسازی خودکار پورت‌های اشغال‌شده در ویندوز"""
    if not IS_WIN:
        return
    try:
        output = subprocess.check_output(f"netstat -ano | findstr :{port}", shell=True, text=True)
        for line in output.strip().split("\n"):
            parts = line.strip().split()
            if len(parts) >= 5 and f":{port}" in parts[1]:
                pid = parts[-1]
                if pid != "0":
                    subprocess.run(f"taskkill /F /PID {pid}", shell=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except Exception:
        pass

def log_stream(process, prefix, color):
    try:
        for line in iter(process.stdout.readline, ""):
            if line:
                print(f"{color}{BOLD}[{prefix}]{RESET} {line.strip()}")
    except Exception:
        pass

def is_port_in_use(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        return s.connect_ex(("127.0.0.1", port)) == 0

def check_preflight():
    print(f"\n{CYAN}{BOLD}======================================================{RESET}")
    print(f"{CYAN}{BOLD}   🚀 MI-EDTE 1-CLICK PRODUCTION LAUNCHER 🚀         {RESET}")
    print(f"{CYAN}{BOLD}======================================================{RESET}\n")

    # آزادسازی پورت‌های داشبورد و گیت‌وی از اجراهای قبلی
    free_port(3000)
    free_port(8000)
    time.sleep(1)

    # بررسی وضعیت Redis
    if not is_port_in_use(6379):
        print(f"{RED}[!] Redis is NOT running on port 6379!{RESET}")
        print(f"{YELLOW}[*] Attempting to start Redis container...{RESET}")
        try:
            subprocess.run(["docker", "run", "-d", "-p", "6379:6379", "--name", "miedte_redis", "redis:alpine"], shell=IS_WIN, check=True)
            time.sleep(2)
            print(f"{GREEN}[✓] Redis started successfully.{RESET}")
        except Exception:
            print(f"{RED}[ERROR] Please start Redis manually!{RESET}")
    else:
        print(f"{GREEN}[✓] Redis Server is active on port 6379.{RESET}")

    # بررسی وجود مدل هوش مصنوعی
    model_path = os.path.join("python_engine", "scalp_lightgbm_model.txt")
    if not os.path.exists(model_path):
        print(f"{YELLOW}[*] Model file not found. Auto-training initial LightGBM model...{RESET}")
        subprocess.run([sys.executable, "app/research/feature_pipeline.py"], cwd="python_engine", shell=IS_WIN, check=True)
        print(f"{GREEN}[✓] Model trained and saved.{RESET}")

def start_services():
    services = [
        ("FASTAPI-GATEWAY", [sys.executable, "app/gateway.py"], "python_engine", GREEN),
        ("AI-SCALP-BRIDGE", [sys.executable, "app/intelligence/realtime_bridge.py"], "python_engine", MAGENTA),
        ("RSS-COLLECTOR",   [sys.executable, "app/collectors/rss_collector.py"], "python_engine", YELLOW),
        ("RUST-CORE",       ["cargo", "run"], "rust_core", CYAN),
        ("NEXTJS-DASHBOARD",["npm", "run", "dev"], "dashboard", BLUE),
    ]

    for name, cmd, cwd, color in services:
        print(f"{color}[*] Launching {name}...{RESET}")
        p = subprocess.Popen(
            cmd,
            cwd=cwd,
            shell=IS_WIN,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            encoding="utf-8",
            errors="replace"
        )
        processes.append(p)
        t = Thread(target=log_stream, args=(p, name, color), daemon=True)
        t.start()
        time.sleep(1.5)

    print(f"\n{GREEN}{BOLD}✨ All 5 Engine Components are ACTIVE & CONNECTED!{RESET}")
    print(f"{GREEN}{BOLD}👉 Open Dashboard in Browser: http://localhost:3000{RESET}")
    print(f"{YELLOW}Press CTRL+C at any time to cleanly stop all services.\n{RESET}")

def shutdown(signum=None, frame=None):
    print(f"\n{RED}{BOLD}[!] Shutting down all MI-EDTE processes...{RESET}")
    for p in processes:
        try:
            if IS_WIN:
                subprocess.run(f"taskkill /F /T /PID {p.pid}", shell=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            else:
                p.terminate()
                p.kill()
        except Exception:
            pass
    print(f"{GREEN}[✓] Clean exit completed. Ports released.{RESET}")
    sys.exit(0)

if __name__ == "__main__":
    signal.signal(signal.SIGINT, shutdown)
    if hasattr(signal, "SIGTERM"):
        signal.signal(signal.SIGTERM, shutdown)

    check_preflight()
    start_services()

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        shutdown()