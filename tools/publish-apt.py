#!/usr/bin/env python3
import gzip, hashlib, io, os, shutil, subprocess, sys, tarfile, time
from pathlib import Path
out = Path(os.environ.get("HAVEN_APT_DIR", Path.home() / "ai/amni-bake/releases/apt"))
gnupg = os.environ.get("HAVEN_APT_GNUPG", str(Path.home() / ".config/amni/signing/flatpak-gnupg"))
url = os.environ.get("HAVEN_APT_URL", "https://downloads.amni-scient.com/apt")
keep = int(os.environ.get("HAVEN_APT_KEEP", "3"))
env = {**os.environ, "GNUPGHOME": gnupg}
key = next(l.split(":")[9] for l in subprocess.run(["gpg", "--list-keys", "--with-colons"], env=env, capture_output=True, text=True, check=True).stdout.splitlines() if l.startswith("fpr"))
def control(deb):
    data = deb.read_bytes()
    pos, members = 8, {}
    while pos < len(data):
        name, size = data[pos:pos + 16].decode().strip().rstrip("/"), int(data[pos + 48:pos + 58].decode().strip())
        members[name], pos = data[pos + 60:pos + 60 + size], pos + 60 + size + (size & 1)
    blob = next(v for k, v in members.items() if k.startswith("control.tar"))
    blob = subprocess.run(["zstd", "-dc"], input=blob, capture_output=True, check=True).stdout if blob[:4] == b"\x28\xb5\x2f\xfd" else blob
    with tarfile.open(fileobj=io.BytesIO(blob)) as t:
        return t.extractfile(next(m for m in t.getmembers() if m.name.lstrip("./") == "control")).read().decode().strip()
def digests(b):
    return {"MD5sum": hashlib.md5(b).hexdigest(), "SHA1": hashlib.sha1(b).hexdigest(), "SHA256": hashlib.sha256(b).hexdigest()}
pool = out / "pool/main/h/haven-desktop"
pool.mkdir(parents=True, exist_ok=True)
for d in map(Path, sys.argv[1:]):
    fields = dict(l.split(": ", 1) for l in control(d).splitlines() if ": " in l and not l.startswith(" "))
    shutil.copy2(d, pool / f"{fields['Package']}_{fields['Version']}_{fields['Architecture']}.deb")
debs = sorted(pool.glob("*.deb"), key=lambda p: p.stat().st_mtime)
[p.unlink() for p in debs[:-keep]]
stanzas = []
for d in sorted(pool.glob("*.deb")):
    b = d.read_bytes()
    h = digests(b)
    stanzas.append(f"{control(d)}\nFilename: {d.relative_to(out)}\nSize: {len(b)}\nMD5sum: {h['MD5sum']}\nSHA1: {h['SHA1']}\nSHA256: {h['SHA256']}\n")
dist = out / "dists/stable"
bin_dir = dist / "main/binary-amd64"
bin_dir.mkdir(parents=True, exist_ok=True)
pk = "\n".join(stanzas).encode()
(bin_dir / "Packages").write_bytes(pk)
(bin_dir / "Packages.gz").write_bytes(gzip.compress(pk, mtime=0))
files = [(f.relative_to(dist).as_posix(), f.read_bytes()) for f in sorted(bin_dir.iterdir())]
release = "\n".join([
    "Origin: Amni-Scient", "Label: Amni-Scient", "Suite: stable", "Codename: stable", "Architectures: amd64", "Components: main",
    "Description: Haven Desktop and other Amni-Scient apps", "Date: " + time.strftime("%a, %d %b %Y %H:%M:%S UTC", time.gmtime()),
    *[f"{algo}:\n" + "\n".join(f" {digests(b)[algo]} {len(b)} {n}" for n, b in files) for algo in ("MD5sum", "SHA1", "SHA256")]]) + "\n"
(dist / "Release").write_text(release.replace("MD5sum:", "MD5Sum:", 1))
for f in ("InRelease", "Release.gpg"):
    (dist / f).unlink(missing_ok=True)
subprocess.run(["gpg", "--batch", "--yes", "--local-user", key, "--clearsign", "-o", str(dist / "InRelease"), str(dist / "Release")], env=env, check=True)
subprocess.run(["gpg", "--batch", "--yes", "--local-user", key, "-abs", "-o", str(dist / "Release.gpg"), str(dist / "Release")], env=env, check=True)
(out / "amni-scient.gpg").write_bytes(subprocess.run(["gpg", "--export", key], env=env, capture_output=True, check=True).stdout)
(out / "amni-scient.sources").write_text(f"Types: deb\nURIs: {url}\nSuites: stable\nComponents: main\nArchitectures: amd64\nSigned-By: /etc/apt/keyrings/amni-scient.gpg\n")
(out / "install.sh").write_text(f"#!/bin/sh\nset -e\nsudo install -d -m 0755 /etc/apt/keyrings\ncurl -fsSL {url}/amni-scient.gpg | sudo tee /etc/apt/keyrings/amni-scient.gpg >/dev/null\ncurl -fsSL {url}/amni-scient.sources | sudo tee /etc/apt/sources.list.d/amni-scient.sources >/dev/null\nsudo apt-get update\nsudo apt-get install -y haven\n")
print(f"apt repo at {out}: {len(stanzas)} package(s)")
