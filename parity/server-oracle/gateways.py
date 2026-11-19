"""Server oracle for the End's return gateways: starts the official server for
a seed, loads the chunks where Chunkscope says gateways are, and asks the
server whether there is an end gateway block entity at the predicted block.

usage: gateways.py <java> <server.jar> <workdir> <seed> <x,y,z> [<x,y,z> ...]
prints one line per position: `<seed> <x> <y> <z> yes|no`. A position is
`yes` when the server has an end gateway block entity there.
"""
import os
import queue
import subprocess
import sys
import threading
import time


def main():
    java, jar, work, seed = sys.argv[1:5]
    positions = [tuple(int(v) for v in p.split(",")) for p in sys.argv[5:]]
    os.makedirs(work, exist_ok=True)
    with open(os.path.join(work, "eula.txt"), "w") as f:
        f.write("eula=true\n")
    with open(os.path.join(work, "server.properties"), "w") as f:
        f.write(
            f"level-seed={seed}\nonline-mode=false\nserver-port=25598\n"
            "view-distance=4\nsimulation-distance=4\nspawn-protection=0\n"
        )
    server = subprocess.Popen(
        [java, "-Xmx2G", "-jar", jar, "--nogui"],
        cwd=work,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    lines = queue.Queue()

    def reader():
        for line in server.stdout:
            lines.put(line.rstrip("\n"))
        lines.put(None)

    threading.Thread(target=reader, daemon=True).start()

    def wait_for(predicate, timeout):
        end = time.time() + timeout
        while time.time() < end:
            try:
                line = lines.get(timeout=1)
            except queue.Empty:
                continue
            if line is None:
                raise SystemExit("server exited")
            if predicate(line):
                return line
        raise SystemExit("timeout")

    def send(command):
        server.stdin.write(command + "\n")
        server.stdin.flush()

    try:
        run(server, positions, seed, wait_for, send)
    finally:
        if server.poll() is None:
            server.kill()


def run(server, positions, seed, wait_for, send):
    wait_for(lambda l: "Done (" in l, 600)
    for x, y, z in positions:
        send(f"execute in minecraft:the_end run forceload add {x} {z}")
        wait_for(lambda l: "marked" in l.lower(), 60)
        # Wait until the chunk is generated and the block entity can be read.
        answer = "no"
        for _ in range(60):
            send(f"execute in minecraft:the_end run data get block {x} {y} {z}")
            line = wait_for(lambda l: "block data" in l or "not a block entity" in l or "not loaded" in l.lower() or "unloaded" in l.lower(), 30)
            if "block data" in line:
                answer = "yes" if "end_gateway" in line.lower() or "ExitPortal" in line or "ExactTeleport" in line else "no"
                break
            if "not a block entity" in line:
                answer = "no"
                time.sleep(1)
                # Keep asking for a few seconds: the chunk may still be generating.
                if _ >= 6:
                    break
                continue
            time.sleep(1)
        print(f"{seed} {x} {y} {z} {answer}", flush=True)
    send("stop")
    server.wait(timeout=120)


main()
